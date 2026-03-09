/*
 * dns_shim.c — standalone reimplementation of hev-mapped-dns
 *
 * Replicates the observable behaviour of hev_mapped_dns_{find,handle,lookup}
 * without any submodule dependencies (no HevRBTree, HevList, hev-task-system).
 *
 * Data-structure swap:
 *   RBTree  → flat array with linear scan  (O(n), acceptable for max ≤ 256)
 *   HevList → doubly-linked list through slot indices (intrusive, no allocation)
 *
 * All semantics match the reference implementation:
 *   - find()   : LRU-touch on hit; evict oldest on miss when full
 *   - handle() : DNS wire-format parse; A-record answer injection; flag patching
 *   - lookup() : reverse ip → name; LRU-touch on hit
 */

#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <arpa/inet.h>

#define DNS_SHIM_MAX_Q 32

/* -------------------------------------------------------------------------
 * Internal data structures
 * ---------------------------------------------------------------------- */

typedef struct
{
    char *name;   /* strdup'd name, NULL if slot is free */
    int   active; /* 1 if occupied, 0 if free            */
    int   prev;   /* LRU prev (older entry), -1 = head   */
    int   next;   /* LRU next (newer entry), -1 = tail   */
} DnsShimSlot;

typedef struct
{
    DnsShimSlot *slots;    /* flat array [max]                   */
    int          max;      /* maximum number of entries          */
    int          use;      /* number of currently active entries */
    uint32_t     net;      /* network prefix (e.g. 0x0a000000)  */
    uint32_t     mask;     /* network mask   (e.g. 0xffffff00)  */
    int          lru_head; /* index of oldest (LRU front), -1 = empty */
    int          lru_tail; /* index of newest (MRU back),  -1 = empty */
} DnsShim;

/* -------------------------------------------------------------------------
 * LRU list helpers
 * ---------------------------------------------------------------------- */

/* Remove slot[idx] from the doubly-linked LRU list. */
static void
lru_remove (DnsShim *shim, int idx)
{
    DnsShimSlot *s = &shim->slots[idx];

    if (s->prev != -1)
        shim->slots[s->prev].next = s->next;
    else
        shim->lru_head = s->next;

    if (s->next != -1)
        shim->slots[s->next].prev = s->prev;
    else
        shim->lru_tail = s->prev;

    s->prev = -1;
    s->next = -1;
}

/* Append slot[idx] to the MRU tail of the LRU list. */
static void
lru_push_tail (DnsShim *shim, int idx)
{
    DnsShimSlot *s = &shim->slots[idx];

    s->prev = shim->lru_tail;
    s->next = -1;

    if (shim->lru_tail != -1)
        shim->slots[shim->lru_tail].next = idx;
    else
        shim->lru_head = idx;

    shim->lru_tail = idx;
}

/* -------------------------------------------------------------------------
 * Public API
 * ---------------------------------------------------------------------- */

DnsShim *
dns_shim_new (uint32_t net, uint32_t mask, int max)
{
    DnsShim *shim;

    if (max <= 0)
        return NULL;

    shim = (DnsShim *)malloc (sizeof (DnsShim));
    if (!shim)
        return NULL;

    shim->slots = (DnsShimSlot *)calloc ((size_t)max, sizeof (DnsShimSlot));
    if (!shim->slots) {
        free (shim);
        return NULL;
    }

    for (int i = 0; i < max; i++) {
        shim->slots[i].name   = NULL;
        shim->slots[i].active = 0;
        shim->slots[i].prev   = -1;
        shim->slots[i].next   = -1;
    }

    shim->max      = max;
    shim->use      = 0;
    shim->net      = net;
    shim->mask     = mask;
    shim->lru_head = -1;
    shim->lru_tail = -1;

    return shim;
}

void
dns_shim_free (DnsShim *shim)
{
    if (!shim)
        return;
    for (int i = 0; i < shim->max; i++) {
        if (shim->slots[i].name)
            free (shim->slots[i].name);
    }
    free (shim->slots);
    free (shim);
}

/*
 * dns_shim_find — look up or insert name, returning the mapped IP index.
 *
 * Returns idx (≥ 0) on success, -1 on allocation failure.
 * Mirrors hev_mapped_dns_find() exactly.
 */
static int
dns_shim_find (DnsShim *shim, const char *name)
{
    int i;

    /* Cache hit: linear scan for matching active slot. */
    for (i = 0; i < shim->max; i++) {
        if (shim->slots[i].active && strcmp (shim->slots[i].name, name) == 0) {
            /* LRU touch: move to MRU tail. */
            lru_remove (shim, i);
            lru_push_tail (shim, i);
            return i;
        }
    }

    /* Cache miss: allocate or evict. */
    if (shim->use < shim->max) {
        /* Pre-fill: use the next free slot (index == use before increment). */
        i = shim->use;
        shim->use++;
    } else {
        /* Full: evict the LRU-front (oldest) entry. */
        i = shim->lru_head;
        lru_remove (shim, i);
        free (shim->slots[i].name);
        shim->slots[i].name   = NULL;
        shim->slots[i].active = 0;
    }

    shim->slots[i].name = strdup (name);
    if (!shim->slots[i].name)
        return -1;

    shim->slots[i].active = 1;
    lru_push_tail (shim, i);

    return i;
}

/*
 * dns_shim_handle — process a DNS request and produce a mapped response.
 *
 * Mirrors hev_mapped_dns_handle() byte-for-byte:
 *   - Copies req → res
 *   - Parses questions; for each A/IN query calls dns_shim_find()
 *   - Appends answer records
 *   - Patches flags (QR=1, RA = RD>>1) and ANCOUNT
 *
 * Returns response length on success, -1 on error.
 */
int
dns_shim_handle (DnsShim *shim, void *req, int qlen, void *res, int slen)
{
    uint8_t *rb = (uint8_t *)req;
    uint8_t *sb = (uint8_t *)res;
    uint16_t fl;
    uint16_t qd;
    int      ips[DNS_SHIM_MAX_Q];
    int      ipo[DNS_SHIM_MAX_Q];
    int      ipn = 0;
    int      off, i;

    if (slen < qlen)
        return -1;

    memcpy (res, req, (size_t)qlen);

    /* QDCOUNT from bytes [4..6] big-endian → host-endian (mirrors ntohs). */
    qd  = (uint16_t)(((uint16_t)rb[4] << 8) | rb[5]);
    /* Flags from bytes [2..3] big-endian → host-endian. */
    fl  = (uint16_t)(((uint16_t)rb[2] << 8) | rb[3]);

    /* Zero NSCOUNT, ANCOUNT, ARCOUNT in response (bytes 6..12). */
    sb[6]  = 0; sb[7]  = 0; /* ANCOUNT */
    sb[8]  = 0; sb[9]  = 0; /* NSCOUNT */
    sb[10] = 0; sb[11] = 0; /* ARCOUNT */

    if (qd > 32)
        return -1;

    off = 12; /* sizeof(DNSHdr) */
    for (i = 0; i < (int)qd; i++) {
        ipo[ipn] = off;

        /* Walk labels, replacing each length byte with '.' in rb. */
        while (rb[off]) {
            int poff = off;

            off += 1 + rb[off];
            if (off >= qlen)
                return -1;

            rb[poff] = '.';
        }

        off++; /* skip null terminator */
        if ((off + 3) >= qlen)
            return -1;

        /* A record (QTYPE=1) in IN class (QCLASS=1)? */
        if ((((uint16_t)rb[off] << 8) | rb[off + 1]) == 1 &&
            (((uint16_t)rb[off + 2] << 8) | rb[off + 3]) == 1)
        {
            /*
             * The name starts at ipo[ipn]+1:
             *   rb[ipo[ipn]] was the first label-length byte, now replaced with '.'
             *   rb[ipo[ipn]+1] is the first character of the first label
             * After mutation the region looks like "example.com\0" as a C string.
             */
            int idx = dns_shim_find (shim, (const char *)&rb[ipo[ipn] + 1]);
            if (idx >= 0) {
                ips[ipn] = (int)(shim->net | (uint32_t)idx);
                ipn++;
            }
        }

        off += 4;
    }

    /* Append answer records to the response buffer. */
    for (i = 0; i < ipn; i++) {
        if ((off + 15) >= slen)
            return -1;

        sb[off + 0] = 0xc0;
        sb[off + 1] = (uint8_t)ipo[i];
        /* TYPE = A (1) */
        sb[off + 2] = 0; sb[off + 3] = 1;
        /* CLASS = IN (1) */
        sb[off + 4] = 0; sb[off + 5] = 1;
        /* TTL = 1 */
        sb[off + 6] = 0; sb[off + 7] = 0; sb[off + 8] = 0; sb[off + 9] = 1;
        /* RDLENGTH = 4 */
        sb[off + 10] = 0; sb[off + 11] = 4;
        /* RDATA: IP in big-endian */
        sb[off + 12] = (uint8_t)(ips[i] >> 24);
        sb[off + 13] = (uint8_t)(ips[i] >> 16);
        sb[off + 14] = (uint8_t)(ips[i] >> 8);
        sb[off + 15] = (uint8_t)(ips[i]);

        off += 16;
    }

    /* Patch flags: QR=1, RA = (RD >> 1). C: fl |= 0x8000 | ((fl & 0x100) >> 1) */
    fl = (uint16_t)(fl | 0x8000u | ((fl & 0x0100u) >> 1));
    sb[2] = (uint8_t)(fl >> 8);
    sb[3] = (uint8_t)(fl);

    /* ANCOUNT = ipn */
    sb[6] = (uint8_t)(ipn >> 8);
    sb[7] = (uint8_t)(ipn);

    return off;
}

/*
 * dns_shim_lookup — reverse lookup: IP → name.
 *
 * Returns the name for a mapped IP, or NULL if not found.
 * LRU-touches the entry on hit (mirrors hev_mapped_dns_lookup).
 */
const char *
dns_shim_lookup (DnsShim *shim, uint32_t ip)
{
    int idx;

    if ((ip & shim->mask) != shim->net)
        return NULL;

    idx = (int)(ip & ~shim->mask);
    if (idx >= shim->max)
        return NULL;

    if (!shim->slots[idx].active)
        return NULL;

    /* LRU touch */
    lru_remove (shim, idx);
    lru_push_tail (shim, idx);

    return shim->slots[idx].name;
}

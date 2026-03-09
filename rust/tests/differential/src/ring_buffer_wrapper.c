/*
 * Thin wrapper around hev-ring-buffer.c for Rust FFI differential tests.
 *
 * HevRingBuffer uses a flexible array member (data[0]), so it must be
 * heap-allocated as sizeof(HevRingBuffer) + capacity bytes.
 *
 * Exposed functions mirror the Rust helpers write_bytes / read_available
 * so the proptest harness can call both sides identically.
 */

#include <stdlib.h>
#include <string.h>
#include <sys/uio.h>

#include "hev-ring-buffer.h"

/* Allocate a new C ring buffer with the given capacity. */
HevRingBuffer *
rb_new (size_t capacity)
{
    HevRingBuffer *rb = malloc (sizeof (HevRingBuffer) + capacity);
    if (!rb)
        return NULL;
    rb->rp = 0;
    rb->wp = 0;
    rb->rda_size = 0;
    rb->use_size = 0;
    rb->max_size = capacity;
    return rb;
}

/* Free a ring buffer allocated by rb_new. */
void
rb_free (HevRingBuffer *rb)
{
    free (rb);
}

/* Write up to len bytes from data into rb using the two-phase write protocol.
 * Returns the number of bytes actually written (may be < len when full). */
size_t
rb_write_bytes (HevRingBuffer *rb, const unsigned char *data, size_t len)
{
    struct iovec iov[2];
    int n;
    size_t total = 0;
    int i;

    n = hev_ring_buffer_writing (rb, iov);
    if (n == 0)
        return 0;

    for (i = 0; i < n && total < len; i++) {
        size_t to_copy = iov[i].iov_len;
        if (to_copy > len - total)
            to_copy = len - total;
        memcpy (iov[i].iov_base, data + total, to_copy);
        total += to_copy;
    }

    hev_ring_buffer_write_finish (rb, total);
    return total;
}

/* Read all rda_size bytes from rb into out (caller must provide enough space).
 * Calls read_finish + read_release for the full rda_size.
 * Returns the number of bytes read. */
size_t
rb_read_available (HevRingBuffer *rb, unsigned char *out, size_t max_out)
{
    struct iovec iov[2];
    int n;
    size_t total = 0;
    size_t rda;
    int i;

    n = hev_ring_buffer_reading (rb, iov);
    if (n == 0)
        return 0;

    rda = rb->rda_size;

    for (i = 0; i < n; i++) {
        size_t to_copy = iov[i].iov_len;
        if (to_copy > max_out - total)
            to_copy = max_out - total;
        memcpy (out + total, iov[i].iov_base, to_copy);
        total += to_copy;
    }

    hev_ring_buffer_read_finish (rb, rda);
    hev_ring_buffer_read_release (rb, rda);
    return total;
}

/* State accessors */
size_t rb_get_max_size (HevRingBuffer *rb) { return rb->max_size; }
size_t rb_get_use_size (HevRingBuffer *rb) { return rb->use_size; }
size_t rb_get_rda_size (HevRingBuffer *rb) { return rb->rda_size; }

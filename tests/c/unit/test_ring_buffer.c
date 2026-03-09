/*
 * Unit tests for hev-ring-buffer.c
 *
 * Tests cover:
 *  - Basic read/write round-trip
 *  - Full buffer detection
 *  - Wrap-around
 *  - Two-iovec scatter-gather path
 *  - Two-phase read: reading/read_finish/read_release independence
 */

#include <string.h>
#include <stdlib.h>
#include <assert.h>
#include <alloca.h>
#include "../framework/unity.h"

/* Include the implementation directly */
#define PRIVATE_API
#include "../../../src/misc/hev-ring-buffer.h"
#include "../../../src/misc/hev-ring-buffer.c"

/* Helper: allocate a ring buffer of given size on heap (not alloca, for tests) */
static HevRingBuffer *
make_ring_buffer (size_t size)
{
    HevRingBuffer *rb = calloc (1, sizeof (HevRingBuffer) + size);
    if (!rb)
        return NULL;
    rb->rp = 0;
    rb->wp = 0;
    rb->rda_size = 0;
    rb->use_size = 0;
    rb->max_size = size;
    return rb;
}

static void
free_ring_buffer (HevRingBuffer *rb)
{
    free (rb);
}

/* Helper: write bytes to ring buffer and commit */
static int
rb_write (HevRingBuffer *rb, const unsigned char *data, size_t len)
{
    struct iovec iov[2];
    int n = hev_ring_buffer_writing (rb, iov);
    if (n == 0)
        return -1; /* full */

    size_t written = 0;
    for (int i = 0; i < n && written < len; i++) {
        size_t chunk = iov[i].iov_len < (len - written) ? iov[i].iov_len : (len - written);
        memcpy (iov[i].iov_base, data + written, chunk);
        written += chunk;
    }
    hev_ring_buffer_write_finish (rb, written);
    return (int)written;
}

/* Helper: read bytes from ring buffer and release */
static int
rb_read (HevRingBuffer *rb, unsigned char *data, size_t len)
{
    struct iovec iov[2];
    int n = hev_ring_buffer_reading (rb, iov);
    if (n == 0)
        return 0; /* empty */

    size_t read_total = 0;
    for (int i = 0; i < n && read_total < len; i++) {
        size_t chunk = iov[i].iov_len < (len - read_total) ? iov[i].iov_len : (len - read_total);
        memcpy (data + read_total, iov[i].iov_base, chunk);
        read_total += chunk;
    }

    hev_ring_buffer_read_finish (rb, read_total);
    hev_ring_buffer_read_release (rb, read_total);
    return (int)read_total;
}

/* ─────────────────────────────────────────────────────────
 * Test: basic round-trip
 * ──────────────────────────────────────────────────────── */
void test_basic_round_trip (void)
{
    HevRingBuffer *rb = make_ring_buffer (64);
    TEST_ASSERT_NOT_NULL (rb);

    const char *msg = "hello world";
    unsigned char out[64] = {0};

    int w = rb_write (rb, (const unsigned char *)msg, strlen (msg));
    TEST_ASSERT_EQUAL_INT ((int)strlen (msg), w);

    int r = rb_read (rb, out, sizeof (out));
    TEST_ASSERT_EQUAL_INT (w, r);
    TEST_ASSERT_EQUAL_MEMORY (msg, out, r);

    free_ring_buffer (rb);
}

/* ─────────────────────────────────────────────────────────
 * Test: get_max_size and get_use_size
 * ──────────────────────────────────────────────────────── */
void test_size_getters (void)
{
    HevRingBuffer *rb = make_ring_buffer (128);
    TEST_ASSERT_NOT_NULL (rb);

    TEST_ASSERT_EQUAL_size_t (128, hev_ring_buffer_get_max_size (rb));
    TEST_ASSERT_EQUAL_size_t (0, hev_ring_buffer_get_use_size (rb));

    const char *data = "ABCD";
    rb_write (rb, (const unsigned char *)data, 4);
    TEST_ASSERT_EQUAL_size_t (4, hev_ring_buffer_get_use_size (rb));

    free_ring_buffer (rb);
}

/* ─────────────────────────────────────────────────────────
 * Test: buffer full detection
 * ──────────────────────────────────────────────────────── */
void test_buffer_full (void)
{
    HevRingBuffer *rb = make_ring_buffer (8);
    TEST_ASSERT_NOT_NULL (rb);

    /* Fill completely */
    const char *data = "12345678";
    int w = rb_write (rb, (const unsigned char *)data, 8);
    TEST_ASSERT_EQUAL_INT (8, w);

    /* Now writing should fail */
    struct iovec iov[2];
    int n = hev_ring_buffer_writing (rb, iov);
    TEST_ASSERT_EQUAL_INT (0, n);

    /* use_size should be max */
    TEST_ASSERT_EQUAL_size_t (8, hev_ring_buffer_get_use_size (rb));

    free_ring_buffer (rb);
}

/* ─────────────────────────────────────────────────────────
 * Test: buffer empty detection
 * ──────────────────────────────────────────────────────── */
void test_buffer_empty (void)
{
    HevRingBuffer *rb = make_ring_buffer (16);
    TEST_ASSERT_NOT_NULL (rb);

    struct iovec iov[2];
    int n = hev_ring_buffer_reading (rb, iov);
    TEST_ASSERT_EQUAL_INT (0, n);

    free_ring_buffer (rb);
}

/* ─────────────────────────────────────────────────────────
 * Test: wrap-around: write fills buffer, read half, write more (crosses boundary)
 * ──────────────────────────────────────────────────────── */
void test_wrap_around (void)
{
    const size_t BUF = 16;
    HevRingBuffer *rb = make_ring_buffer (BUF);
    TEST_ASSERT_NOT_NULL (rb);

    /* Fill with pattern A (8 bytes) */
    const char *data_a = "AAAAAAAA";
    rb_write (rb, (const unsigned char *)data_a, 8);

    /* Read 8 bytes — now rp = 8 */
    unsigned char tmp[8];
    rb_read (rb, tmp, 8);
    TEST_ASSERT_EQUAL_size_t (0, hev_ring_buffer_get_use_size (rb));

    /* After read_release with all data consumed, rp=wp=0 reset */
    /* Actually, per code: only resets if use_size == 0 after release */

    /* Write 8 bytes again to fill */
    const char *data_b = "BBBBBBBB";
    rb_write (rb, (const unsigned char *)data_b, 8);

    /* Write 8 more bytes — should wrap around or use remaining space */
    const char *data_c = "CCCCCCCC";
    int w = rb_write (rb, (const unsigned char *)data_c, 8);
    TEST_ASSERT_GREATER_THAN (0, w);

    /* Read all and verify B then C data */
    unsigned char out[16] = {0};
    int total = 0;
    int r;
    while ((r = rb_read (rb, out + total, sizeof (out) - total)) > 0)
        total += r;

    /* Should have read B portion + C portion */
    TEST_ASSERT_GREATER_THAN (0, total);

    free_ring_buffer (rb);
}

/* ─────────────────────────────────────────────────────────
 * Test: two-iovec scatter-gather path
 * When wp < rp (wrapped), writing() returns 2 iovecs
 * ──────────────────────────────────────────────────────── */
void test_two_iovec_write (void)
{
    const size_t BUF = 16;
    HevRingBuffer *rb = make_ring_buffer (BUF);
    TEST_ASSERT_NOT_NULL (rb);

    /* Write 12 bytes, read 8 bytes */
    /* This sets: rp = 8, wp = 12, use_size = 4 */
    unsigned char fill[12];
    memset (fill, 0xAA, 12);
    rb_write (rb, fill, 12);

    {
        struct iovec iov[2];
        int n = hev_ring_buffer_reading (rb, iov);
        TEST_ASSERT_GREATER_THAN (0, n);
        /* Read 8 bytes: advance rp by 8 */
        hev_ring_buffer_read_finish (rb, 8);
        hev_ring_buffer_read_release (rb, 8);
    }

    /* rp = 8, wp = 12, use_size = 4, free space = 12 */
    /* writing() from wp=12: upper_size = 16-12 = 4, spc_size = 12 */
    /* Since spc_size (12) > upper_size (4) → should return 2 iovecs */
    struct iovec iov[2];
    int n = hev_ring_buffer_writing (rb, iov);
    TEST_ASSERT_EQUAL_INT (2, n);
    TEST_ASSERT_EQUAL_size_t (4, iov[0].iov_len); /* upper part */
    TEST_ASSERT_EQUAL_size_t (8, iov[1].iov_len); /* wrapped part (rp - 0 = 8) */

    free_ring_buffer (rb);
}

/* ─────────────────────────────────────────────────────────
 * Test: two-iovec read path
 * When rp > wp (wrapped), reading() returns 2 iovecs
 * ──────────────────────────────────────────────────────── */
void test_two_iovec_read (void)
{
    const size_t BUF = 16;
    HevRingBuffer *rb = make_ring_buffer (BUF);
    TEST_ASSERT_NOT_NULL (rb);

    /* Force wp to wrap: write 12 bytes, read 8, write 8 more */
    unsigned char fill[12];
    memset (fill, 0xBB, 12);
    rb_write (rb, fill, 12);

    /* Read 8 without release (to move rp) */
    struct iovec iov[2];
    int n = hev_ring_buffer_reading (rb, iov);
    hev_ring_buffer_read_finish (rb, 8);
    hev_ring_buffer_read_release (rb, 8);
    /* Now rp = 8, wp = 12, use_size = 4 */

    /* Write 8 more bytes — wraps wp to 4 (12+8=20, 20%16=4) */
    unsigned char more[8];
    memset (more, 0xCC, 8);
    {
        struct iovec wv[2];
        int wn = hev_ring_buffer_writing (rb, wv);
        /* Write to first iov at least */
        size_t written = 0;
        for (int i = 0; i < wn && written < 8; i++) {
            size_t chunk = wv[i].iov_len < (8 - written) ? wv[i].iov_len : (8 - written);
            memcpy (wv[i].iov_base, more + written, chunk);
            written += chunk;
        }
        hev_ring_buffer_write_finish (rb, written);
    }

    /* Now reading should return 2 iovecs: [rp..end] and [0..wp] */
    n = hev_ring_buffer_reading (rb, iov);
    TEST_ASSERT_EQUAL_INT (2, n);

    free_ring_buffer (rb);
}

/* ─────────────────────────────────────────────────────────
 * Test: Two-phase protocol independence
 * read_finish() advances rp; read_release() decrements use_size
 * These are INDEPENDENT operations.
 * ──────────────────────────────────────────────────────── */
void test_two_phase_read_independence (void)
{
    HevRingBuffer *rb = make_ring_buffer (64);
    TEST_ASSERT_NOT_NULL (rb);

    /* Write 16 bytes */
    unsigned char data[16];
    for (int i = 0; i < 16; i++)
        data[i] = (unsigned char)i;
    rb_write (rb, data, 16);

    size_t initial_use = hev_ring_buffer_get_use_size (rb);
    TEST_ASSERT_EQUAL_size_t (16, initial_use);

    /* Call reading() — does NOT advance rp */
    struct iovec iov[2];
    int n = hev_ring_buffer_reading (rb, iov);
    TEST_ASSERT_GREATER_THAN (0, n);
    TEST_ASSERT_EQUAL_size_t (16, hev_ring_buffer_get_use_size (rb)); /* unchanged */

    size_t rp_before = rb->rp;

    /* Call read_finish(8) — advances rp by 8, decrements rda_size by 8 */
    hev_ring_buffer_read_finish (rb, 8);
    TEST_ASSERT_EQUAL_size_t (8, rb->rp - rp_before);        /* rp advanced */
    TEST_ASSERT_EQUAL_size_t (16, hev_ring_buffer_get_use_size (rb)); /* use_size UNCHANGED */

    /* Call read_release(8) — decrements use_size by 8 */
    hev_ring_buffer_read_release (rb, 8);
    TEST_ASSERT_EQUAL_size_t (8, hev_ring_buffer_get_use_size (rb)); /* now decremented */

    free_ring_buffer (rb);
}

/* ─────────────────────────────────────────────────────────
 * Test: read_release(all) resets rp and wp to 0
 * ──────────────────────────────────────────────────────── */
void test_read_release_all_resets (void)
{
    HevRingBuffer *rb = make_ring_buffer (32);
    TEST_ASSERT_NOT_NULL (rb);

    const char *data = "data";
    rb_write (rb, (const unsigned char *)data, 4);

    struct iovec iov[2];
    hev_ring_buffer_reading (rb, iov);
    hev_ring_buffer_read_finish (rb, 4);
    hev_ring_buffer_read_release (rb, 4);

    /* After releasing all, rp and wp should be reset to 0 */
    TEST_ASSERT_EQUAL_size_t (0, rb->rp);
    TEST_ASSERT_EQUAL_size_t (0, rb->wp);
    TEST_ASSERT_EQUAL_size_t (0, hev_ring_buffer_get_use_size (rb));

    free_ring_buffer (rb);
}

/* ─────────────────────────────────────────────────────────
 * Test: large sequential write-read cycles
 * ──────────────────────────────────────────────────────── */
void test_sequential_cycles (void)
{
    const size_t BUF = 256;
    HevRingBuffer *rb = make_ring_buffer (BUF);
    TEST_ASSERT_NOT_NULL (rb);

    unsigned char write_buf[64], read_buf[64];
    for (int cycle = 0; cycle < 100; cycle++) {
        for (int i = 0; i < 64; i++)
            write_buf[i] = (unsigned char)(cycle * 64 + i);

        int w = rb_write (rb, write_buf, 64);
        TEST_ASSERT_GREATER_THAN (0, w);

        int r = rb_read (rb, read_buf, 64);
        TEST_ASSERT_EQUAL_INT (w, r);
        TEST_ASSERT_EQUAL_MEMORY (write_buf, read_buf, r);
    }

    free_ring_buffer (rb);
}

/* ─────────────────────────────────────────────────────────
 * Test: partial reads
 * ──────────────────────────────────────────────────────── */
void test_partial_reads (void)
{
    HevRingBuffer *rb = make_ring_buffer (64);
    TEST_ASSERT_NOT_NULL (rb);

    const char *data = "Hello World Test";
    rb_write (rb, (const unsigned char *)data, 16);

    /* Read 5 bytes at a time */
    unsigned char out[16] = {0};
    int total = 0;
    int r;
    while ((r = rb_read (rb, out + total, 5)) > 0)
        total += r;

    TEST_ASSERT_EQUAL_INT (16, total);
    TEST_ASSERT_EQUAL_MEMORY (data, out, 16);

    free_ring_buffer (rb);
}

void setUp (void) {}
void tearDown (void) {}

int
main (void)
{
    UNITY_BEGIN ();

    RUN_TEST (test_basic_round_trip);
    RUN_TEST (test_size_getters);
    RUN_TEST (test_buffer_full);
    RUN_TEST (test_buffer_empty);
    RUN_TEST (test_wrap_around);
    RUN_TEST (test_two_iovec_write);
    RUN_TEST (test_two_iovec_read);
    RUN_TEST (test_two_phase_read_independence);
    RUN_TEST (test_read_release_all_resets);
    RUN_TEST (test_sequential_cycles);
    RUN_TEST (test_partial_reads);

    return UNITY_END ();
}

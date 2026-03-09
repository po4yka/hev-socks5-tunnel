/*
 * Unit tests for hev-logger.c
 */

#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <unistd.h>
#include <fcntl.h>
#include "../framework/unity.h"
#include "../../../src/misc/hev-logger.h"
#include "../../../src/misc/hev-logger.c"

#define TMP_LOG_FILE "/tmp/hev_logger_test.log"

void setUp (void) {}
void tearDown (void)
{
    hev_logger_fini ();
    unlink (TMP_LOG_FILE);
}

void test_init_stderr (void)
{
    int res = hev_logger_init (HEV_LOGGER_DEBUG, "stderr");
    TEST_ASSERT_EQUAL_INT (0, res);
    /* fd should be valid */
    TEST_ASSERT_GREATER_OR_EQUAL (0, fd);
    hev_logger_fini ();
}

void test_init_stdout (void)
{
    int res = hev_logger_init (HEV_LOGGER_INFO, "stdout");
    TEST_ASSERT_EQUAL_INT (0, res);
    hev_logger_fini ();
}

void test_init_file (void)
{
    int res = hev_logger_init (HEV_LOGGER_DEBUG, TMP_LOG_FILE);
    TEST_ASSERT_EQUAL_INT (0, res);
    hev_logger_fini ();

    /* File should exist */
    TEST_ASSERT_EQUAL_INT (0, access (TMP_LOG_FILE, F_OK));
}

void test_level_filtering_debug_passes_all (void)
{
    hev_logger_init (HEV_LOGGER_DEBUG, TMP_LOG_FILE);

    TEST_ASSERT_EQUAL_INT (1, hev_logger_enabled (HEV_LOGGER_DEBUG));
    TEST_ASSERT_EQUAL_INT (1, hev_logger_enabled (HEV_LOGGER_INFO));
    TEST_ASSERT_EQUAL_INT (1, hev_logger_enabled (HEV_LOGGER_WARN));
    TEST_ASSERT_EQUAL_INT (1, hev_logger_enabled (HEV_LOGGER_ERROR));
}

void test_level_filtering_info_blocks_debug (void)
{
    hev_logger_init (HEV_LOGGER_INFO, TMP_LOG_FILE);

    TEST_ASSERT_EQUAL_INT (0, hev_logger_enabled (HEV_LOGGER_DEBUG));
    TEST_ASSERT_EQUAL_INT (1, hev_logger_enabled (HEV_LOGGER_INFO));
    TEST_ASSERT_EQUAL_INT (1, hev_logger_enabled (HEV_LOGGER_WARN));
    TEST_ASSERT_EQUAL_INT (1, hev_logger_enabled (HEV_LOGGER_ERROR));
}

void test_level_filtering_warn_blocks_debug_info (void)
{
    hev_logger_init (HEV_LOGGER_WARN, TMP_LOG_FILE);

    TEST_ASSERT_EQUAL_INT (0, hev_logger_enabled (HEV_LOGGER_DEBUG));
    TEST_ASSERT_EQUAL_INT (0, hev_logger_enabled (HEV_LOGGER_INFO));
    TEST_ASSERT_EQUAL_INT (1, hev_logger_enabled (HEV_LOGGER_WARN));
    TEST_ASSERT_EQUAL_INT (1, hev_logger_enabled (HEV_LOGGER_ERROR));
}

void test_level_filtering_error_only (void)
{
    hev_logger_init (HEV_LOGGER_ERROR, TMP_LOG_FILE);

    TEST_ASSERT_EQUAL_INT (0, hev_logger_enabled (HEV_LOGGER_DEBUG));
    TEST_ASSERT_EQUAL_INT (0, hev_logger_enabled (HEV_LOGGER_INFO));
    TEST_ASSERT_EQUAL_INT (0, hev_logger_enabled (HEV_LOGGER_WARN));
    TEST_ASSERT_EQUAL_INT (1, hev_logger_enabled (HEV_LOGGER_ERROR));
}

void test_log_writes_to_file (void)
{
    hev_logger_init (HEV_LOGGER_DEBUG, TMP_LOG_FILE);
    hev_logger_log (HEV_LOGGER_INFO, "test message %d", 42);
    hev_logger_fini ();

    /* Read the file and check content */
    FILE *f = fopen (TMP_LOG_FILE, "r");
    TEST_ASSERT_NOT_NULL (f);

    char buf[512] = {0};
    size_t n = fread (buf, 1, sizeof (buf) - 1, f);
    fclose (f);

    TEST_ASSERT_GREATER_THAN (0, (int)n);
    /* Should contain level marker and message */
    TEST_ASSERT_NOT_NULL (strstr (buf, "[I]"));
    TEST_ASSERT_NOT_NULL (strstr (buf, "test message 42"));
}

void test_log_debug_marker (void)
{
    hev_logger_init (HEV_LOGGER_DEBUG, TMP_LOG_FILE);
    hev_logger_log (HEV_LOGGER_DEBUG, "debug msg");
    hev_logger_fini ();

    FILE *f = fopen (TMP_LOG_FILE, "r");
    char buf[512] = {0};
    fread (buf, 1, sizeof (buf) - 1, f);
    fclose (f);

    TEST_ASSERT_NOT_NULL (strstr (buf, "[D]"));
}

void test_log_warn_marker (void)
{
    hev_logger_init (HEV_LOGGER_DEBUG, TMP_LOG_FILE);
    hev_logger_log (HEV_LOGGER_WARN, "warn msg");
    hev_logger_fini ();

    FILE *f = fopen (TMP_LOG_FILE, "r");
    char buf[512] = {0};
    fread (buf, 1, sizeof (buf) - 1, f);
    fclose (f);

    TEST_ASSERT_NOT_NULL (strstr (buf, "[W]"));
}

void test_log_error_marker (void)
{
    hev_logger_init (HEV_LOGGER_DEBUG, TMP_LOG_FILE);
    hev_logger_log (HEV_LOGGER_ERROR, "error msg");
    hev_logger_fini ();

    FILE *f = fopen (TMP_LOG_FILE, "r");
    char buf[512] = {0};
    fread (buf, 1, sizeof (buf) - 1, f);
    fclose (f);

    TEST_ASSERT_NOT_NULL (strstr (buf, "[E]"));
}

void test_filtered_message_not_written (void)
{
    hev_logger_init (HEV_LOGGER_WARN, TMP_LOG_FILE);
    hev_logger_log (HEV_LOGGER_DEBUG, "should not appear");
    hev_logger_log (HEV_LOGGER_INFO, "should not appear either");
    hev_logger_fini ();

    FILE *f = fopen (TMP_LOG_FILE, "r");
    char buf[512] = {0};
    size_t n = fread (buf, 1, sizeof (buf) - 1, f);
    fclose (f);

    /* File should be empty (nothing was logged) */
    TEST_ASSERT_EQUAL_size_t (0, n);
}

void test_log_format_includes_timestamp (void)
{
    hev_logger_init (HEV_LOGGER_DEBUG, TMP_LOG_FILE);
    hev_logger_log (HEV_LOGGER_INFO, "ts check");
    hev_logger_fini ();

    FILE *f = fopen (TMP_LOG_FILE, "r");
    char buf[512] = {0};
    fread (buf, 1, sizeof (buf) - 1, f);
    fclose (f);

    /* Timestamp format: [YYYY-MM-DD HH:MM:SS] */
    TEST_ASSERT_EQUAL_CHAR ('[', buf[0]);
    /* Should contain year like 20xx */
    TEST_ASSERT_NOT_NULL (strstr (buf, "20"));
}

int
main (void)
{
    UNITY_BEGIN ();

    RUN_TEST (test_init_stderr);
    RUN_TEST (test_init_stdout);
    RUN_TEST (test_init_file);
    RUN_TEST (test_level_filtering_debug_passes_all);
    RUN_TEST (test_level_filtering_info_blocks_debug);
    RUN_TEST (test_level_filtering_warn_blocks_debug_info);
    RUN_TEST (test_level_filtering_error_only);
    RUN_TEST (test_log_writes_to_file);
    RUN_TEST (test_log_debug_marker);
    RUN_TEST (test_log_warn_marker);
    RUN_TEST (test_log_error_marker);
    RUN_TEST (test_filtered_message_not_written);
    RUN_TEST (test_log_format_includes_timestamp);

    return UNITY_END ();
}

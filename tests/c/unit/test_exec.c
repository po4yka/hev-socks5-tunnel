/*
 * Unit tests for hev-exec.c
 */

#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <unistd.h>
#include <signal.h>
#include <sys/wait.h>
#include "../framework/unity.h"

/* Provide a no-op logger for exec dependency */
#include "../../../src/misc/hev-logger.h"

/* Stub logger implementation for testing exec */
/* We need to provide logger for exec.c dependency. */

/* Include with a no-op logger that won't output anything */
#include "../../../src/misc/hev-logger.c"
#include "../../../src/misc/hev-exec.c"

#define TMP_SCRIPT "/tmp/hev_exec_test_script.sh"
#define TMP_OUTPUT "/tmp/hev_exec_test_output.txt"

void setUp (void)
{
    /* Initialize logger to suppress output */
    hev_logger_init (HEV_LOGGER_ERROR, "stderr");
}

void tearDown (void)
{
    hev_logger_fini ();
    unlink (TMP_SCRIPT);
    unlink (TMP_OUTPUT);
}

/* Helper: create a test script that writes its arguments to a file */
static void
create_test_script (const char *output_file)
{
    FILE *f = fopen (TMP_SCRIPT, "w");
    if (!f)
        return;

    fprintf (f, "#!/bin/sh\n");
    fprintf (f, "echo \"$1 $2\" > %s\n", output_file);
    fclose (f);
    chmod (TMP_SCRIPT, 0755);
}

/* Helper: read the output file */
static int
read_output (char *buf, size_t maxlen)
{
    FILE *f = fopen (TMP_OUTPUT, "r");
    if (!f)
        return -1;
    size_t n = fread (buf, 1, maxlen - 1, f);
    buf[n] = '\0';
    fclose (f);
    return (int)n;
}

void test_exec_run_with_wait (void)
{
    create_test_script (TMP_OUTPUT);

    /* Run script with wait=1 (synchronous) */
    hev_exec_run (TMP_SCRIPT, "tun0", "3", 1);

    /* After wait, output file should exist */
    char buf[128] = {0};
    int n = read_output (buf, sizeof (buf));

    TEST_ASSERT_GREATER_THAN (0, n);
    /* Output should contain tun_name and tun_index */
    TEST_ASSERT_NOT_NULL (strstr (buf, "tun0"));
    TEST_ASSERT_NOT_NULL (strstr (buf, "3"));
}

void test_exec_run_without_wait (void)
{
    create_test_script (TMP_OUTPUT);

    /* Run script with wait=0 (async) */
    hev_exec_run (TMP_SCRIPT, "tun1", "5", 0);

    /* Wait a moment for the child to complete */
    usleep (100000); /* 100ms */

    /* Reap any children */
    while (waitpid (-1, NULL, WNOHANG) > 0)
        ;

    char buf[128] = {0};
    int n = read_output (buf, sizeof (buf));
    TEST_ASSERT_GREATER_THAN (0, n);
    TEST_ASSERT_NOT_NULL (strstr (buf, "tun1"));
}

void test_exec_run_nonexistent_script (void)
{
    /* Running a non-existent script should not crash the process */
    /* Child will fail to exec, but parent continues */
    hev_exec_run ("/nonexistent/path/script.sh", "tun0", "0", 1);

    /* If we reach here, no crash occurred */
    TEST_ASSERT_TRUE (1);
}

void test_exec_run_passes_tun_name (void)
{
    create_test_script (TMP_OUTPUT);
    hev_exec_run (TMP_SCRIPT, "my_tunnel", "99", 1);

    char buf[128] = {0};
    read_output (buf, sizeof (buf));
    TEST_ASSERT_NOT_NULL (strstr (buf, "my_tunnel"));
}

void test_exec_run_passes_tun_index (void)
{
    create_test_script (TMP_OUTPUT);
    hev_exec_run (TMP_SCRIPT, "tun0", "42", 1);

    char buf[128] = {0};
    read_output (buf, sizeof (buf));
    TEST_ASSERT_NOT_NULL (strstr (buf, "42"));
}

int
main (void)
{
    UNITY_BEGIN ();

    RUN_TEST (test_exec_run_with_wait);
    RUN_TEST (test_exec_run_without_wait);
    RUN_TEST (test_exec_run_nonexistent_script);
    RUN_TEST (test_exec_run_passes_tun_name);
    RUN_TEST (test_exec_run_passes_tun_index);

    return UNITY_END ();
}

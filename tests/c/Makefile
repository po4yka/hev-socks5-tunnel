# Makefile for hev-socks5-tunnel C unit tests
#
# Usage:
#   make         - build and run all tests
#   make test    - run all tests
#   make SANITIZERS=1  - build with ASAN+UBSAN

CC      ?= gcc
SRCDIR  := ../../src
FWDIR   := framework

CFLAGS  := -std=c11 -Wall -Wextra -g \
           -D_POSIX_C_SOURCE=200809L \
           -Wno-unused-parameter \
           -I$(SRCDIR)/misc \
           -I$(SRCDIR) \
           -I$(FWDIR)

ifeq ($(SANITIZERS),1)
CFLAGS  += -fsanitize=address,undefined -fno-omit-frame-pointer
endif

UNITY_SRC := $(FWDIR)/unity.c

TESTS := \
    test_ring_buffer \
    test_list \
    test_logger \
    test_exec

.PHONY: all test clean

all: $(TESTS)

test: all
	@echo "=== Running C unit tests ==="
	@failed=0; total=0; \
	for t in $(TESTS); do \
	    total=$$((total+1)); \
	    echo "--- $$t ---"; \
	    ./$$t; \
	    if [ $$? -ne 0 ]; then failed=$$((failed+1)); fi; \
	done; \
	echo ""; \
	echo "=== Results: $$((total-failed))/$$total passed ==="; \
	[ $$failed -eq 0 ]

test_ring_buffer: unit/test_ring_buffer.c $(UNITY_SRC)
	$(CC) $(CFLAGS) -o $@ $^

test_list: unit/test_list.c $(UNITY_SRC)
	$(CC) $(CFLAGS) -o $@ $^

test_logger: unit/test_logger.c $(UNITY_SRC)
	$(CC) $(CFLAGS) -o $@ $^

test_exec: unit/test_exec.c $(UNITY_SRC)
	$(CC) $(CFLAGS) -o $@ $^

clean:
	rm -f $(TESTS)
	rm -f /tmp/hev_logger_test.log /tmp/hev_exec_test_*.sh /tmp/hev_exec_test_output.txt

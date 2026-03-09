/*
 * Unit tests for hev-list.c (doubly-linked list)
 */

#include <string.h>
#include "../framework/unity.h"
#include "../../../src/misc/hev-list.h"
#include "../../../src/misc/hev-list.c"

typedef struct
{
    HevListNode node;
    int value;
} TestNode;

void setUp (void) {}
void tearDown (void) {}

void test_add_tail_single (void)
{
    HevList list = { NULL, NULL };
    TestNode n1 = { { NULL, NULL }, 42 };

    hev_list_add_tail (&list, &n1.node);

    TEST_ASSERT_EQUAL_PTR (&n1.node, list.head);
    TEST_ASSERT_EQUAL_PTR (&n1.node, list.tail);
    TEST_ASSERT_NULL (n1.node.prev);
    TEST_ASSERT_NULL (n1.node.next);
}

void test_add_tail_multiple (void)
{
    HevList list = { NULL, NULL };
    TestNode n1 = { { NULL, NULL }, 1 };
    TestNode n2 = { { NULL, NULL }, 2 };
    TestNode n3 = { { NULL, NULL }, 3 };

    hev_list_add_tail (&list, &n1.node);
    hev_list_add_tail (&list, &n2.node);
    hev_list_add_tail (&list, &n3.node);

    TEST_ASSERT_EQUAL_PTR (&n1.node, list.head);
    TEST_ASSERT_EQUAL_PTR (&n3.node, list.tail);

    /* Forward traversal */
    HevListNode *cur = hev_list_first (&list);
    TEST_ASSERT_EQUAL_PTR (&n1.node, cur);
    cur = hev_list_node_next (cur);
    TEST_ASSERT_EQUAL_PTR (&n2.node, cur);
    cur = hev_list_node_next (cur);
    TEST_ASSERT_EQUAL_PTR (&n3.node, cur);
    cur = hev_list_node_next (cur);
    TEST_ASSERT_NULL (cur);

    /* Backward traversal */
    cur = hev_list_last (&list);
    TEST_ASSERT_EQUAL_PTR (&n3.node, cur);
    cur = hev_list_node_prev (cur);
    TEST_ASSERT_EQUAL_PTR (&n2.node, cur);
    cur = hev_list_node_prev (cur);
    TEST_ASSERT_EQUAL_PTR (&n1.node, cur);
    cur = hev_list_node_prev (cur);
    TEST_ASSERT_NULL (cur);
}

void test_del_head (void)
{
    HevList list = { NULL, NULL };
    TestNode n1 = { { NULL, NULL }, 1 };
    TestNode n2 = { { NULL, NULL }, 2 };
    TestNode n3 = { { NULL, NULL }, 3 };

    hev_list_add_tail (&list, &n1.node);
    hev_list_add_tail (&list, &n2.node);
    hev_list_add_tail (&list, &n3.node);

    hev_list_del (&list, &n1.node);

    TEST_ASSERT_EQUAL_PTR (&n2.node, list.head);
    TEST_ASSERT_EQUAL_PTR (&n3.node, list.tail);
    TEST_ASSERT_NULL (n2.node.prev);
}

void test_del_tail (void)
{
    HevList list = { NULL, NULL };
    TestNode n1 = { { NULL, NULL }, 1 };
    TestNode n2 = { { NULL, NULL }, 2 };

    hev_list_add_tail (&list, &n1.node);
    hev_list_add_tail (&list, &n2.node);

    hev_list_del (&list, &n2.node);

    TEST_ASSERT_EQUAL_PTR (&n1.node, list.head);
    TEST_ASSERT_EQUAL_PTR (&n1.node, list.tail);
    TEST_ASSERT_NULL (n1.node.next);
}

void test_del_middle (void)
{
    HevList list = { NULL, NULL };
    TestNode n1 = { { NULL, NULL }, 1 };
    TestNode n2 = { { NULL, NULL }, 2 };
    TestNode n3 = { { NULL, NULL }, 3 };

    hev_list_add_tail (&list, &n1.node);
    hev_list_add_tail (&list, &n2.node);
    hev_list_add_tail (&list, &n3.node);

    hev_list_del (&list, &n2.node);

    TEST_ASSERT_EQUAL_PTR (&n1.node, list.head);
    TEST_ASSERT_EQUAL_PTR (&n3.node, list.tail);
    TEST_ASSERT_EQUAL_PTR (&n3.node, n1.node.next);
    TEST_ASSERT_EQUAL_PTR (&n1.node, n3.node.prev);
}

void test_del_only_node (void)
{
    HevList list = { NULL, NULL };
    TestNode n1 = { { NULL, NULL }, 1 };

    hev_list_add_tail (&list, &n1.node);
    hev_list_del (&list, &n1.node);

    TEST_ASSERT_NULL (list.head);
    TEST_ASSERT_NULL (list.tail);
}

void test_list_empty_initial (void)
{
    HevList list = { NULL, NULL };
    TEST_ASSERT_NULL (hev_list_first (&list));
    TEST_ASSERT_NULL (hev_list_last (&list));
}

void test_iteration_count (void)
{
    HevList list = { NULL, NULL };
    TestNode nodes[10];
    for (int i = 0; i < 10; i++) {
        nodes[i].node.next = NULL;
        nodes[i].node.prev = NULL;
        nodes[i].value = i;
        hev_list_add_tail (&list, &nodes[i].node);
    }

    int count = 0;
    for (HevListNode *n = hev_list_first (&list); n; n = hev_list_node_next (n))
        count++;

    TEST_ASSERT_EQUAL_INT (10, count);
}

int
main (void)
{
    UNITY_BEGIN ();

    RUN_TEST (test_add_tail_single);
    RUN_TEST (test_add_tail_multiple);
    RUN_TEST (test_del_head);
    RUN_TEST (test_del_tail);
    RUN_TEST (test_del_middle);
    RUN_TEST (test_del_only_node);
    RUN_TEST (test_list_empty_initial);
    RUN_TEST (test_iteration_count);

    return UNITY_END ();
}

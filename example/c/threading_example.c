#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <pthread.h>
#include <threading_example.h>

void init(MembraneHandle *handle)
{
    setbuf(stdout, NULL);
    pthread_t thread_id = 0;

    printf("\n[call_async_c] [C] Spawning detached thread\n");
    pthread_detach(thread_id);
    pthread_create(&thread_id, NULL, supervisor, (void *)handle);
    printf("\n[call_async_c] [C] Done spawning detached thread\n");
}

void *supervisor(void *vptr)
{
    printf("\n[call_async_c] [C] Worker supervisor is running \n");

    pthread_t thread_one_id;
    pthread_t thread_two_id;
    pthread_create(&thread_one_id, NULL, worker, (void *)vptr);
    pthread_create(&thread_two_id, NULL, worker, (void *)vptr);
    pthread_join(thread_one_id, NULL);
    pthread_join(thread_two_id, NULL);

    // we hand the pointer back to Rust so it can do cleanup
    membrane_drop_handle(vptr);

    pthread_exit(NULL);
}

void *worker(void *vptr)
{
    MembraneHandle handle = *(MembraneHandle *)vptr;

    int i = 0;
    char buffer[100];
    unsigned long tid = (unsigned long)pthread_self();

    usleep(5000);
    printf("\n[call_async_c] [C] This is running in detached C thread %lu after sleeping for 5ms \n", tid);

    while (!handle.is_done(handle.is_done_ctx))
    {
        i++;
        sprintf(buffer, "This is a string from a C thread: Thread %lu, Count %d", tid, i);
        handle.push(handle.push_ctx, buffer);
        printf("[call_async_c] [C] %s\n", buffer);
        usleep(50);
    }
    printf("[call_async_c] [C] stream was closed, Thread %lu shutting down\n", tid);

    pthread_exit(NULL);
}

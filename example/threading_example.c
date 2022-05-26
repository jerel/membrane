#include <stdio.h>
#include <stdlib.h>
#include <unistd.h> //Header file for sleep(). man 3 sleep for details.
#include <pthread.h>
#include <threading_example.h>

void init(MembraneHandle *handle)
{
    setbuf(stdout, NULL);
    pthread_t thread_id;

    printf("\n[call_c] [C] Spawning detached thread\n");
    pthread_detach(thread_id);
    pthread_create(&thread_id, NULL, supervisor, (void *)handle);
    printf("\n[call_c] [C] Done spawning detached thread\n");
}

void *supervisor(void *vptr)
{
    printf("\n[call_c] [C] Worker supervisor is running \n");

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

    sleep(1);
    printf("\n[call_c] [C] This is running in a detached C thread after sleeping for 1 second \n");

    char string[] = "This is a string from a C thread";
    handle.push(handle.context, &string);

    pthread_exit(NULL);
}

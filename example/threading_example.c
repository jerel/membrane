#include <stdio.h>
#include <stdlib.h>
#include <unistd.h> //Header file for sleep(). man 3 sleep for details.
#include <pthread.h>

typedef void *Context;

typedef void *Data;

typedef struct membrane_handle MembraneHandle;

struct membrane_handle
{
    Context context;
    void (*push)(Context, Data);
    void (*drop)(Context);
};

void *worker(void *vptr)
{
    MembraneHandle handle = *(MembraneHandle *)vptr;

    sleep(1);
    printf("\n[call_c] [C] This is running in a detached C thread after sleeping for 1 second \n");

    char first[] = "This is a string from the first";
    handle.push(handle.context, &first);

    char second[] = "This is the string from the second";
    handle.push(handle.context, &second);

    handle.drop(handle.context);

    pthread_exit(NULL);
}

int init(MembraneHandle *handle)
{
    setbuf(stdout, NULL);
    pthread_t thread_id;

    printf("\n[call_c] [C] Spawning detached thread\n");
    pthread_detach(thread_id);
    pthread_create(&thread_id, NULL, worker, (void *)handle);
    printf("\n[call_c] [C] Done spawning detached thread\n");

    return 1;
}

typedef void *Context;

typedef void *Data;

typedef struct MembraneHandle
{
    Context context;
    void (*push)(Context, Data);
    void (*drop)(Context);
} MembraneHandle;

void *worker(MembraneHandle);
int init(*MembraneHandle);

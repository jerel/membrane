typedef void *Context;

typedef void *Data;

typedef struct MembraneHandle
{
    Context context;
    void (*push)(Context, Data);
    void (*drop)(Context);
} MembraneHandle;

void membrane_drop_handle(void *);

void *worker(void *);
void *supervisor(void *);
int init(MembraneHandle *);

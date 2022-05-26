
// Every C program must contain the below API at a minimum.
// The `init` function can be named anything (it is called from Rust)
// and it is free to take additional arguments if desired.

typedef void *Context;
typedef void *Data;
typedef struct MembraneHandle
{
    Context context;
    void (*push)(Context, Data);
} MembraneHandle;

void init(MembraneHandle *);
void membrane_drop_handle(MembraneHandle *);

// example threading implementation, unrelated to Membrane
void *worker(void *);
void *supervisor(void *);

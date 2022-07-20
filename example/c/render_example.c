#include <stdio.h>
#include <render_example.h>

void print_via_c(char *value)
{
    setbuf(stdout, NULL);

    printf("\n[render_via_c] [C] %s\n", value);
}

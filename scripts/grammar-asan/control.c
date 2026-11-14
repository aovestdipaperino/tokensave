#include <stdlib.h>

int main(int argc, char **argv) {
    (void)argv;
    volatile char *bytes = (volatile char *)malloc(8);
    if (bytes == NULL) return 2;
    bytes[8 + argc] = 1;
    free((void *)bytes);
    return 0;
}

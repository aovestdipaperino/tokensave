/**
 * Sample C file exercising all extractor features.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define MAX_BUFFER_SIZE 1024
#define MIN(a, b) ((a) < (b) ? (a) : (b))

/* Represents a 2D point. */
typedef struct {
    double x;
    double y;
} Point;

struct Color {
    unsigned char r;
    unsigned char g;
    unsigned char b;
    unsigned char a;
};

/* A tagged union for variant data. */
union Variant {
    int int_val;
    float float_val;
    char str_val[64];
};

typedef union Variant Variant;

/* Status codes for operations. */
enum Status {
    STATUS_OK = 0,
    STATUS_ERROR = -1,
    STATUS_PENDING = 1,
    STATUS_TIMEOUT = 2,
};

typedef enum Status Status;

/* Function pointer for callbacks. */
typedef void (*Callback)(int code, const char *message);

/* Global error buffer. */
static char error_buffer[MAX_BUFFER_SIZE];

/* Last recorded status. */
int last_status = STATUS_OK;

/* Compute the distance between two points. */
double point_distance(const Point *a, const Point *b) {
    double dx = a->x - b->x;
    double dy = a->y - b->y;
    return sqrt(dx * dx + dy * dy);
}

/* Create a new point. */
Point point_new(double x, double y) {
    Point p;
    p.x = x;
    p.y = y;
    return p;
}

// Internal helper — not exported.
static void set_error(const char *msg) {
    strncpy(error_buffer, msg, MAX_BUFFER_SIZE - 1);
    error_buffer[MAX_BUFFER_SIZE - 1] = '\0';
}

/* Process a variant value with a callback. */
void process_variant(const union Variant *v, Callback cb) {
    if (cb != NULL) {
        cb(v->int_val, "processed");
    }
    printf("Variant int value: %d\n", v->int_val);
    set_error("none");
}

/* Entry point. */
int main(int argc, char *argv[]) {
    Point origin = point_new(0.0, 0.0);
    Point target = point_new(3.0, 4.0);
    double dist = point_distance(&origin, &target);
    printf("Distance: %f\n", dist);

    union Variant v;
    v.int_val = 42;
    process_variant(&v, NULL);

    return last_status;
}

/**
 * Sample C header file exercising declaration/prototype extraction.
 */

#define API_VERSION 2

/* A rectangle defined by origin and size. */
typedef struct {
    int x;
    int y;
    int width;
    int height;
} Rect;

enum LogLevel {
    LOG_DEBUG,
    LOG_INFO,
    LOG_WARN,
    LOG_ERROR,
};

/* Create a new rectangle. */
Rect rect_new(int x, int y, int w, int h);

/* Compute the area of a rectangle. */
int rect_area(const Rect *r);

/* Check if a point is inside a rectangle. */
int rect_contains(const Rect *r, int px, int py);

/* Initialize the logging subsystem. */
void log_init(enum LogLevel level);

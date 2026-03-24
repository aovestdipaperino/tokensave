/**
 * Sample C++ file exercising all extractor features.
 */

#include <iostream>
#include <string>
#include <vector>
#include <memory>

#define DEFAULT_CAPACITY 16

namespace geom {

/// A 2D vector with x and y components.
struct Vec2 {
    double x;
    double y;

    double length() const {
        return std::sqrt(x * x + y * y);
    }
};

/// Abstract base class for all shapes.
class Shape {
public:
    virtual ~Shape() = default;

    /// Compute the area of the shape.
    virtual double area() const = 0;

    /// Compute the perimeter of the shape.
    virtual double perimeter() const = 0;

    std::string name() const { return name_; }

protected:
    explicit Shape(std::string name) : name_(std::move(name)) {}

private:
    std::string name_;
};

/// A circle defined by center and radius.
class Circle : public Shape {
public:
    Circle(Vec2 center, double radius)
        : Shape("circle"), center_(center), radius_(radius) {}

    double area() const override {
        return 3.14159265358979 * radius_ * radius_;
    }

    double perimeter() const override {
        return 2.0 * 3.14159265358979 * radius_;
    }

    Vec2 center() const { return center_; }
    double radius() const { return radius_; }

private:
    Vec2 center_;
    double radius_;
};

/// A rectangle defined by origin, width, and height.
class Rectangle : public Shape {
public:
    Rectangle(Vec2 origin, double w, double h)
        : Shape("rectangle"), origin_(origin), width_(w), height_(h) {}

    ~Rectangle() override = default;

    double area() const override {
        return width_ * height_;
    }

    double perimeter() const override {
        return 2.0 * (width_ + height_);
    }

private:
    Vec2 origin_;
    double width_;
    double height_;
};

/// Generic container with a fixed capacity.
template <typename T>
class FixedBuffer {
public:
    explicit FixedBuffer(size_t capacity = DEFAULT_CAPACITY)
        : capacity_(capacity) {
        data_.reserve(capacity);
    }

    void push(const T& item) {
        if (data_.size() < capacity_) {
            data_.push_back(item);
        }
    }

    size_t size() const { return data_.size(); }

private:
    std::vector<T> data_;
    size_t capacity_;
};

} // namespace geom

enum class Color {
    Red,
    Green,
    Blue,
};

union Number {
    int i;
    float f;
    double d;
};

typedef unsigned long EntityId;

using ShapePtr = std::unique_ptr<geom::Shape>;

/// Print shape info to stdout.
void print_shape(const geom::Shape& shape) {
    std::cout << shape.name()
              << " area=" << shape.area()
              << " perimeter=" << shape.perimeter()
              << std::endl;
}

static void internal_helper() {
    // not exported
}

int main() {
    geom::Vec2 origin{0.0, 0.0};
    geom::Circle circle(origin, 5.0);
    geom::Rectangle rect(origin, 3.0, 4.0);

    print_shape(circle);
    print_shape(rect);

    geom::FixedBuffer<int> buffer(32);
    buffer.push(1);
    buffer.push(2);

    internal_helper();

    return 0;
}

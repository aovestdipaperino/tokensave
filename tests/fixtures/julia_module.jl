module HelloMod

struct Point
    x::Float64
    y::Float64
end

function distance(a::Point, b::Point)
    return sqrt((a.x - b.x)^2 + (a.y - b.y)^2)
end

end

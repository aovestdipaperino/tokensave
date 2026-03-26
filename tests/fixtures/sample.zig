const std = @import("std");
const mem = @import("std").mem;

/// Maximum number of connections allowed.
const max_connections: u32 = 100;

/// Represents a log level.
const LogLevel = enum {
    debug,
    info,
    warning,
    err,
};

/// A 2D point.
const Point = struct {
    x: f64,
    y: f64,

    /// Calculate distance to another point.
    pub fn distance(self: Point, other: Point) f64 {
        const dx = self.x - other.x;
        const dy = self.y - other.y;
        return @sqrt(dx * dx + dy * dy);
    }

    pub fn origin() Point {
        return .{ .x = 0, .y = 0 };
    }
};

/// Manages a network connection.
const Connection = struct {
    host: []const u8,
    port: u16,
    connected: bool,

    /// Creates a new connection.
    pub fn init(host: []const u8, port: u16) Connection {
        return .{
            .host = host,
            .port = port,
            .connected = false,
        };
    }

    /// Establishes the connection.
    pub fn connect(self: *Connection) !void {
        std.debug.print("Connecting to {s}:{d}\n", .{ self.host, self.port });
        self.connected = true;
    }

    pub fn disconnect(self: *Connection) void {
        self.connected = false;
    }

    pub fn isConnected(self: Connection) bool {
        return self.connected;
    }
};

/// Logs a message at the given level.
pub fn log(level: LogLevel, message: []const u8) void {
    _ = level;
    std.debug.print("{s}\n", .{message});
}

/// Processes a list of connections.
pub fn processConnections(connections: []Connection) u32 {
    var count: u32 = 0;
    for (connections) |*conn| {
        conn.connect() catch continue;
        count += 1;
    }
    return count;
}

test "point distance" {
    const p1 = Point{ .x = 0, .y = 0 };
    const p2 = Point{ .x = 3, .y = 4 };
    const d = p1.distance(p2);
    try std.testing.expectEqual(@as(f64, 5.0), d);
}

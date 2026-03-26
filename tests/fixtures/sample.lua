--- @module networking
-- Networking utilities for managing connections.

local json = require("json")
local socket = require("socket")

--- Maximum number of retries.
local MAX_RETRIES = 3

--- Default port for connections.
local DEFAULT_PORT = 8080

--- Logs a message with the given level.
--- @param level string The log level
--- @param message string The message to log
local function log(level, message)
    print(string.format("[%s] %s", level, message))
end

--- Connection class implemented via table.
local Connection = {}
Connection.__index = Connection

--- Creates a new Connection.
--- @param host string The host to connect to
--- @param port number The port number
--- @return Connection
function Connection.new(host, port)
    local self = setmetatable({}, Connection)
    self.host = host
    self.port = port or DEFAULT_PORT
    self.connected = false
    return self
end

--- Connects to the remote host.
function Connection:connect()
    log("INFO", "Connecting to " .. self.host .. ":" .. self.port)
    self.connected = true
    return true
end

--- Disconnects from the remote host.
function Connection:disconnect()
    self.connected = false
end

--- Checks if the connection is active.
function Connection:isConnected()
    return self.connected
end

--- Pool manages multiple connections.
local Pool = {}
Pool.__index = Pool

function Pool.new(host, size)
    local self = setmetatable({}, Pool)
    self.host = host
    self.size = size or 10
    self.connections = {}
    return self
end

function Pool:acquire()
    if #self.connections > 0 then
        return table.remove(self.connections)
    end
    local conn = Connection.new(self.host)
    conn:connect()
    return conn
end

function Pool:release(conn)
    table.insert(self.connections, conn)
end

return {
    Connection = Connection,
    Pool = Pool,
    log = log,
}

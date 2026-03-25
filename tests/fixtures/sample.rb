# Sample Ruby file exercising all extractor features.

module Networking
  # Maximum number of open connections allowed.
  MAX_CONNECTIONS = 100
  # Default timeout in seconds.
  DEFAULT_TIMEOUT = 30

  # Logs a message to stdout.
  def log(message)
    puts message
  end

  # Base class with shared connection functionality.
  class Base
    # Initialize with a host address.
    def initialize(host)
      @host = host
      @connected = false
    end

    # Returns a string representation of the object.
    def to_s
      "#{self.class.name}(#{@host})"
    end

    def self.version
      "1.0"
    end

    private

    # Internal helper to validate state.
    def validate_state
      raise "Invalid state" unless @host
    end
  end

  # Manages a single network connection.
  class Connection < Base
    def initialize(host, port = 8080)
      super(host)
      @port = port
    end

    # Establishes the connection.
    def connect
      log("Connecting to #{@host}:#{@port}")
      @connected = true
    end

    def disconnect
      @connected = false
    end

    def connected?
      @connected
    end

    # Nested configuration class.
    class Config
      def initialize(timeout = DEFAULT_TIMEOUT)
        @timeout = timeout
      end

      def valid?
        @timeout > 0
      end
    end
  end

  # A pool of connections.
  class Pool < Connection
    def initialize(host, size = 10)
      super(host)
      @size = size
      @connections = []
    end

    # Acquires a connection from the pool.
    def acquire
      if @connections.empty?
        conn = Connection.new(@host)
        conn.connect
        conn
      else
        @connections.pop
      end
    end

    def release(conn)
      @connections.push(conn)
    end
  end
end

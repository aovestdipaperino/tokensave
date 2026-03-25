<?php

declare(strict_types=1);

namespace App\Http {

use Psr\Log\LoggerInterface;
use App\Contracts\Connectable;
use RuntimeException;

const DEFAULT_TIMEOUT = 30;

/**
 * Log a message using the provided logger.
 */
function log_message(string $message): void
{
    error_log($message);
}

/**
 * Defines the contract for network connections.
 */
interface ConnectionInterface
{
    /** Open the connection and return success status. */
    public function connect(): bool;

    /** Close the connection. */
    public function disconnect(): void;
}

/**
 * Provides timestamping behaviour for connection events.
 */
trait Timestamps
{
    private \DateTimeImmutable $connectedAt;

    public function markConnected(): void
    {
        $this->connectedAt = new \DateTimeImmutable();
    }
}

/**
 * Provides basic logging capability.
 */
trait Loggable
{
    public function log(string $message): void
    {
        log_message($message);
    }
}

/**
 * Manages a single network connection.
 */
class Connection implements ConnectionInterface
{
    use Timestamps;

    public string $host;
    private int $port;
    protected bool $connected = false;

    /**
     * Create a new connection.
     *
     * @param string $host Remote hostname.
     * @param int    $port Remote port.
     */
    public function __construct(string $host, int $port = 8080)
    {
        $this->host = $host;
        $this->port = $port;
    }

    /** Open the connection. */
    public function connect(): bool
    {
        log_message("Connecting to {$this->host}:{$this->port}");
        $this->markConnected();
        $this->connected = true;
        return true;
    }

    /** Close the connection. */
    public function disconnect(): void
    {
        $this->connected = false;
    }

    private function validatePort(): bool
    {
        return $this->port > 0 && $this->port <= 65535;
    }
}

/**
 * A pooled connection that manages multiple Connection instances.
 */
class Pool extends Connection
{
    use Loggable;

    private int $size;

    public function __construct(string $host, int $size = 10)
    {
        parent::__construct($host);
        $this->size = $size;
    }

    public function acquire(): ?Connection
    {
        $this->log("Acquiring connection from pool");
        $conn = new Connection($this->host);
        $conn->connect();
        return $conn;
    }
}

/** Connection state enumeration. */
enum ConnectionState: string
{
    case Idle      = 'idle';
    case Active    = 'active';
    case Closed    = 'closed';
}

} // end namespace App\Http

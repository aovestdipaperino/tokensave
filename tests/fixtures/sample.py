"""Sample Python file exercising all extractor features."""

import os
from pathlib import Path
from typing import Optional, List

MAX_CONNECTIONS = 100
DEFAULT_TIMEOUT = 30

def log(message: str) -> None:
    """Log a message to stdout."""
    print(message)


def retry(func):
    """Decorator that retries a function up to 3 times."""
    def wrapper(*args, **kwargs):
        for attempt in range(3):
            try:
                return func(*args, **kwargs)
            except Exception:
                if attempt == 2:
                    raise
    return wrapper


class Base:
    """Base class with shared functionality."""

    CLASS_VERSION = "1.0"

    def __init__(self, name: str):
        self._name = name

    def __repr__(self) -> str:
        return f"{self.__class__.__name__}({self._name!r})"

    def _internal_method(self) -> None:
        """Private helper method."""
        pass


class Connection(Base):
    """Manages a network connection."""

    def __init__(self, host: str, port: int = 8080):
        """Initialize connection with host and port."""
        super().__init__(host)
        self.__port = port
        self._connected = False

    @retry
    async def connect(self) -> bool:
        """Establish the connection asynchronously."""
        log(f"Connecting to {self._name}:{self.__port}")
        self._connected = True
        return True

    def disconnect(self) -> None:
        self._connected = False

    @property
    def is_connected(self) -> bool:
        return self._connected

    class Config:
        """Nested configuration class."""
        def __init__(self, timeout: int = DEFAULT_TIMEOUT):
            self.timeout = timeout

        def validate(self) -> bool:
            return self.timeout > 0


class Pool(Connection):
    """Connection pool with multiple inheritance support."""

    def __init__(self, host: str, size: int = 10):
        super().__init__(host)
        self._size = size
        self._connections: List[Connection] = []

    async def acquire(self) -> Optional[Connection]:
        """Acquire a connection from the pool."""
        if self._connections:
            return self._connections.pop()
        conn = Connection(self._name)
        await conn.connect()
        return conn

    def release(self, conn: Connection) -> None:
        self._connections.append(conn)

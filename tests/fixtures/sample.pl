#!/usr/bin/perl
use strict;
use warnings;

use File::Path qw(make_path);
use Carp qw(croak);

# Maximum number of retries.
our $MAX_RETRIES = 3;
# Default port for connections.
our $DEFAULT_PORT = 8080;

# Logs a message with the given level.
sub log_message {
    my ($level, $message) = @_;
    print "[$level] $message\n";
}

package Connection;

# Creates a new Connection object.
sub new {
    my ($class, %args) = @_;
    my $self = bless {
        host      => $args{host},
        port      => $args{port} || $main::DEFAULT_PORT,
        connected => 0,
    }, $class;
    return $self;
}

# Connects to the remote host.
sub connect {
    my ($self) = @_;
    main::log_message("INFO", "Connecting to $self->{host}:$self->{port}");
    $self->{connected} = 1;
    return 1;
}

# Disconnects from the remote host.
sub disconnect {
    my ($self) = @_;
    $self->{connected} = 0;
}

# Checks if the connection is active.
sub is_connected {
    my ($self) = @_;
    return $self->{connected};
}

package Pool;

# Creates a new Pool.
sub new {
    my ($class, %args) = @_;
    my $self = bless {
        host        => $args{host},
        size        => $args{size} || 10,
        connections => [],
    }, $class;
    return $self;
}

# Acquires a connection from the pool.
sub acquire {
    my ($self) = @_;
    if (@{$self->{connections}}) {
        return pop @{$self->{connections}};
    }
    my $conn = Connection->new(host => $self->{host});
    $conn->connect();
    return $conn;
}

sub release {
    my ($self, $conn) = @_;
    push @{$self->{connections}}, $conn;
}

package main;

sub validate_config {
    my ($host, $port) = @_;
    croak "HOST is required" unless $host;
    croak "Invalid port" unless $port > 0 && $port < 65536;
    return 1;
}

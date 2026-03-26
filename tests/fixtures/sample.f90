! Sample Fortran file exercising extractor features.
module networking
    implicit none

    ! Maximum number of retries.
    integer, parameter :: MAX_RETRIES = 3
    ! Default port for connections.
    integer, parameter :: DEFAULT_PORT = 8080

    ! Represents a network endpoint.
    type :: Endpoint
        character(len=256) :: host
        integer :: port
        logical :: connected
    end type Endpoint

    ! Extends Endpoint with pool functionality.
    type, extends(Endpoint) :: PooledEndpoint
        integer :: pool_size
    end type PooledEndpoint

    ! Interface for connectable types.
    interface Connectable
        module procedure connect_endpoint
    end interface Connectable

contains

    ! Logs a message with the given level.
    subroutine log_message(level, message)
        character(len=*), intent(in) :: level
        character(len=*), intent(in) :: message
        print *, '[', trim(level), '] ', trim(message)
    end subroutine log_message

    ! Creates a new endpoint.
    function create_endpoint(host, port) result(ep)
        character(len=*), intent(in) :: host
        integer, intent(in), optional :: port
        type(Endpoint) :: ep

        ep%host = host
        if (present(port)) then
            ep%port = port
        else
            ep%port = DEFAULT_PORT
        end if
        ep%connected = .false.
    end function create_endpoint

    ! Connects an endpoint.
    subroutine connect_endpoint(ep)
        type(Endpoint), intent(inout) :: ep
        call log_message("INFO", "Connecting to " // trim(ep%host))
        ep%connected = .true.
    end subroutine connect_endpoint

    ! Disconnects an endpoint.
    subroutine disconnect_endpoint(ep)
        type(Endpoint), intent(inout) :: ep
        ep%connected = .false.
    end subroutine disconnect_endpoint

    ! Checks if endpoint is connected.
    logical function is_connected(ep)
        type(Endpoint), intent(in) :: ep
        is_connected = ep%connected
    end function is_connected

end module networking

program main
    use networking
    implicit none

    type(Endpoint) :: conn

    conn = create_endpoint("localhost", 8080)
    call connect_endpoint(conn)
    call disconnect_endpoint(conn)
end program main

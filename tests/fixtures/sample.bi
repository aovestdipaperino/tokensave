' QuickBasic 4.5 include file
' Shared declarations for the project

' $DYNAMIC

DECLARE SUB InitSystem ()
DECLARE SUB Shutdown ()
DECLARE FUNCTION GetStatus% ()

CONST VERSION = 45
CONST MAX_ITEMS = 100

TYPE Config
    name AS STRING * 64
    value AS INTEGER
    active AS INTEGER
END TYPE

DIM SHARED appConfig AS Config
DIM SHARED items() AS STRING

' Initializes the system.
SUB InitSystem
    REDIM items(1 TO MAX_ITEMS) AS STRING
    appConfig.name = "QuickBASIC"
    appConfig.value = VERSION
    appConfig.active = 1
    CALL LogInit
END SUB

' Shuts down the system.
SUB Shutdown
    appConfig.active = 0
    ERASE items
    SLEEP 1
END SUB

' Returns the current status.
FUNCTION GetStatus%
    IF appConfig.active = 1 THEN
        GetStatus% = appConfig.value
    ELSE
        GetStatus% = 0
    END IF
END FUNCTION

' Logs initialization.
SUB LogInit
    PRINT "System initialized: "; appConfig.name
END SUB

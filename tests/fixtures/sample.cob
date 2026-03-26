       IDENTIFICATION DIVISION.
       PROGRAM-ID. NETWORKING.
       AUTHOR. TOKENSAVE.

       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.

       DATA DIVISION.
       WORKING-STORAGE SECTION.
      * Maximum number of retries.
       01 WS-MAX-RETRIES        PIC 9(2) VALUE 3.
      * Default port number.
       01 WS-DEFAULT-PORT       PIC 9(5) VALUE 8080.
      * Connection host name.
       01 WS-HOST               PIC X(256).
      * Connection port.
       01 WS-PORT               PIC 9(5).
      * Connection status flag.
       01 WS-CONNECTED          PIC 9 VALUE 0.
      * Log level.
       01 WS-LOG-LEVEL          PIC X(10).
      * Log message text.
       01 WS-LOG-MESSAGE        PIC X(256).
      * Retry counter.
       01 WS-RETRY-COUNT        PIC 9(2) VALUE 0.

       PROCEDURE DIVISION.
       MAIN-PROGRAM.
           PERFORM VALIDATE-CONFIG
           PERFORM CONNECT-SERVER
           PERFORM DISCONNECT-SERVER
           STOP RUN.

      * Validates the configuration.
       VALIDATE-CONFIG.
           IF WS-HOST = SPACES
               MOVE "ERROR" TO WS-LOG-LEVEL
               MOVE "HOST is not set" TO WS-LOG-MESSAGE
               PERFORM LOG-MESSAGE
               STOP RUN
           END-IF.

      * Logs a message with timestamp.
       LOG-MESSAGE.
           DISPLAY "[" WS-LOG-LEVEL "] " WS-LOG-MESSAGE.

      * Connects to the remote server.
       CONNECT-SERVER.
           MOVE "INFO" TO WS-LOG-LEVEL
           STRING "Connecting to " WS-HOST ":" WS-PORT
               DELIMITED BY SIZE INTO WS-LOG-MESSAGE
           PERFORM LOG-MESSAGE
           PERFORM VARYING WS-RETRY-COUNT FROM 1 BY 1
               UNTIL WS-RETRY-COUNT > WS-MAX-RETRIES
               MOVE 1 TO WS-CONNECTED
               IF WS-CONNECTED = 1
                   MOVE "INFO" TO WS-LOG-LEVEL
                   MOVE "Connected" TO WS-LOG-MESSAGE
                   PERFORM LOG-MESSAGE
               END-IF
           END-PERFORM.

      * Disconnects from the server.
       DISCONNECT-SERVER.
           MOVE 0 TO WS-CONNECTED
           MOVE "INFO" TO WS-LOG-LEVEL
           MOVE "Disconnecting" TO WS-LOG-MESSAGE
           PERFORM LOG-MESSAGE.

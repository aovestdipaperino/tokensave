Imports System
Imports System.Collections.Generic

''' <summary>
''' Maximum connections allowed.
''' </summary>
Const MaxConnections As Integer = 100

''' <summary>
''' Represents log severity.
''' </summary>
Enum LogLevel
    Debug
    Info
    Warning
    [Error]
End Enum

''' <summary>
''' Interface for serializable objects.
''' </summary>
Interface ISerializable
    Function ToJson() As String
End Interface

''' <summary>
''' Base class with shared functionality.
''' </summary>
Class Base
    Public ReadOnly Property Name As String

    Sub New(name As String)
        Me.Name = name
    End Sub

    Public Function Description() As String
        Return $"{Me.GetType().Name}({Name})"
    End Function

    Private Sub Validate()
        Debug.Assert(Not String.IsNullOrEmpty(Name))
    End Sub
End Class

''' <summary>
''' Manages a network connection.
''' </summary>
Class Connection
    Inherits Base
    Implements ISerializable

    Public Property Port As Integer
    Private _connected As Boolean = False

    Sub New(host As String, Optional port As Integer = 8080)
        MyBase.New(host)
        Me.Port = port
    End Sub

    Public Sub Connect()
        Console.WriteLine($"Connecting to {Name}:{Port}")
        _connected = True
    End Sub

    Public Sub Disconnect()
        _connected = False
    End Sub

    Public Function IsConnected() As Boolean
        Return _connected
    End Function

    Public Function ToJson() As String Implements ISerializable.ToJson
        Return $"{{""host"":""{Name}"",""port"":{Port}}}"
    End Function
End Class

Structure Point
    Public X As Double
    Public Y As Double

    Function Distance(other As Point) As Double
        Dim dx = X - other.X
        Dim dy = Y - other.Y
        Return Math.Sqrt(dx * dx + dy * dy)
    End Function
End Structure

Module Helpers
    Sub LogMessage(level As LogLevel, message As String)
        Console.WriteLine($"[{level}] {message}")
    End Sub
End Module

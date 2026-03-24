{ Sample Pascal unit exercising all extractor features. }
unit SampleUnit;

interface

uses
  SysUtils, Classes;

const
  MaxRetries = 3;
  AppName = 'SampleApp';

type
  TLogLevel = (llDebug, llInfo, llWarning, llError);

  TPoint = record
    X: Double;
    Y: Double;
  end;

  { Interface for serializable objects. }
  ISerializable = interface
    function ToJSON: string;
  end;

  { Abstract base class for entities. }
  TEntity = class
  private
    FId: string;
    FCreatedAt: TDateTime;
  protected
    function GetId: string;
  public
    constructor Create(const AId: string);
    destructor Destroy; override;
    function Validate: Boolean; virtual; abstract;
    property Id: string read GetId;
    property CreatedAt: TDateTime read FCreatedAt;
  end;

  { A user entity with full feature coverage. }
  TUser = class(TEntity, ISerializable)
  private
    FName: string;
    FEmail: string;
    FLevel: TLogLevel;
    procedure LogAction(const Action: string);
  protected
    FIsActive: Boolean;
  public
    constructor Create(const AId, AName, AEmail: string);
    destructor Destroy; override;
    function Validate: Boolean; override;
    function ToJSON: string;
    function FetchProfile: string;
    property Name: string read FName write FName;
    property Level: TLogLevel read FLevel write FLevel;
  end;

{ Top-level function. }
function PointDistance(const A, B: TPoint): Double;

{ Top-level procedure. }
procedure LogMessage(const Msg: string; Level: TLogLevel);

implementation

{ TEntity }

constructor TEntity.Create(const AId: string);
begin
  FId := AId;
  FCreatedAt := Now;
end;

destructor TEntity.Destroy;
begin
  inherited;
end;

function TEntity.GetId: string;
begin
  Result := FId;
end;

{ TUser }

constructor TUser.Create(const AId, AName, AEmail: string);
begin
  inherited Create(AId);
  FName := AName;
  FEmail := AEmail;
  FLevel := llInfo;
  FIsActive := True;
end;

destructor TUser.Destroy;
begin
  inherited;
end;

function TUser.Validate: Boolean;
begin
  Result := (FName <> '') and (Pos('@', FEmail) > 0);
end;

function TUser.ToJSON: string;
begin
  Result := Format('{"id":"%s","name":"%s"}', [FId, FName]);
end;

function TUser.FetchProfile: string;
begin
  LogAction('fetch_profile');
  WriteLn('Fetched profile for ', FName);
  Result := ToJSON;
end;

procedure TUser.LogAction(const Action: string);
begin
  WriteLn(Format('[%d] %s: %s', [Ord(FLevel), FName, Action]));
end;

{ Top-level routines }

function PointDistance(const A, B: TPoint): Double;
var
  DX, DY: Double;
begin
  DX := A.X - B.X;
  DY := A.Y - B.Y;
  Result := Sqrt(DX * DX + DY * DY);
end;

procedure LogMessage(const Msg: string; Level: TLogLevel);
begin
  WriteLn(Format('[%d] %s', [Ord(Level), Msg]));
end;

end.

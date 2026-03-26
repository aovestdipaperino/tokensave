#import <Foundation/Foundation.h>
#import "Connection.h"

#define MAX_RETRIES 3
#define DEFAULT_PORT 8080

/// Represents log severity.
typedef NS_ENUM(NSInteger, LogLevel) {
    LogLevelDebug,
    LogLevelInfo,
    LogLevelWarning,
    LogLevelError
};

/// Protocol for serializable objects.
@protocol Serializable <NSObject>
- (NSDictionary *)toJson;
- (NSString *)toJsonString;
@end

/// Base class with shared functionality.
@interface Base : NSObject
@property (nonatomic, strong, readonly) NSString *name;
- (instancetype)initWithName:(NSString *)name;
- (NSString *)description;
@end

@implementation Base

- (instancetype)initWithName:(NSString *)name {
    self = [super init];
    if (self) {
        _name = [name copy];
    }
    return self;
}

- (NSString *)description {
    return [NSString stringWithFormat:@"%@(%@)",
            NSStringFromClass([self class]), self.name];
}

/// Private validation helper.
- (void)validate {
    NSAssert(self.name.length > 0, @"Name must not be empty");
}

@end

/// Manages a network connection.
@interface Connection : Base <Serializable>
@property (nonatomic, assign) NSInteger port;
@property (nonatomic, assign, readonly) BOOL connected;
- (instancetype)initWithHost:(NSString *)host port:(NSInteger)port;
- (BOOL)connect;
- (void)disconnect;
+ (instancetype)connectionWithHost:(NSString *)host;
@end

@implementation Connection

- (instancetype)initWithHost:(NSString *)host port:(NSInteger)port {
    self = [super initWithName:host];
    if (self) {
        _port = port;
        _connected = NO;
    }
    return self;
}

- (BOOL)connect {
    NSLog(@"Connecting to %@:%ld", self.name, (long)self.port);
    _connected = YES;
    return YES;
}

- (void)disconnect {
    _connected = NO;
}

+ (instancetype)connectionWithHost:(NSString *)host {
    return [[self alloc] initWithHost:host port:DEFAULT_PORT];
}

- (NSDictionary *)toJson {
    return @{@"host": self.name, @"port": @(self.port)};
}

- (NSString *)toJsonString {
    NSData *data = [NSJSONSerialization dataWithJSONObject:[self toJson] options:0 error:nil];
    return [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding];
}

@end

/// Top-level C function for logging.
void logMessage(LogLevel level, NSString *message) {
    NSLog(@"[%ld] %@", (long)level, message);
}

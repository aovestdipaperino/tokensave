/// Sample Dart file exercising all extractor features.

library sample;

import 'dart:async';
import 'dart:convert';

const int maxRetries = 3;

typedef JsonMap = Map<String, dynamic>;
typedef Callback = void Function(String message);

/// Represents a log level.
enum LogLevel {
  debug,
  info,
  warning,
  error,
}

/// Interface-like abstract class for serializable objects.
abstract class Serializable {
  JsonMap toJson();
  String toJsonString() => jsonEncode(toJson());
}

/// Mixin that adds timestamps to entities.
mixin Timestamped {
  DateTime get createdAt;
  DateTime? get updatedAt;

  Duration age() => DateTime.now().difference(createdAt);
}

/// A user entity with serialization and timestamp support.
class User extends Serializable with Timestamped {
  final String id;
  final String name;
  final String _email;
  LogLevel logLevel;

  @override
  final DateTime createdAt;

  @override
  DateTime? updatedAt;

  User(this.id, this.name, this._email, {this.logLevel = LogLevel.info})
      : createdAt = DateTime.now();

  User.guest()
      : id = '0',
        name = 'Guest',
        _email = 'guest@example.com',
        logLevel = LogLevel.debug,
        createdAt = DateTime.now();

  @override
  JsonMap toJson() => {
        'id': id,
        'name': name,
        'logLevel': logLevel.name,
      };

  /// Fetch the user profile asynchronously.
  Future<JsonMap> fetchProfile() async {
    await Future.delayed(Duration(milliseconds: 100));
    print('Fetched profile for $name');
    return toJson();
  }

  bool get _isValid => name.isNotEmpty && _email.contains('@');

  void _logAction(String action) {
    print('[$logLevel] $name: $action');
  }
}

/// Extension adding utility methods to String.
extension StringUtils on String {
  String toSlug() => toLowerCase().replaceAll(' ', '-');
  bool get isBlank => trim().isEmpty;
}

/// Top-level function that processes users.
Future<List<JsonMap>> processUsers(List<User> users) async {
  final results = <JsonMap>[];
  for (final user in users) {
    final profile = await user.fetchProfile();
    results.add(profile);
  }
  return results;
}

/// Synchronous helper function.
void logMessage(String message, {LogLevel level = LogLevel.info}) {
  print('[${level.name}] $message');
}

/// <summary>
/// Sample C# file exercising all extractor features.
/// </summary>

using System;
using System.Collections.Generic;
using System.Threading.Tasks;

namespace SampleApp.Models
{
    /// <summary>Represents a log level.</summary>
    public enum LogLevel
    {
        Debug,
        Info,
        Warning,
        Error,
    }

    /// <summary>A record for immutable configuration.</summary>
    public record AppConfig(string Name, int MaxRetries, TimeSpan Timeout);

    /// <summary>Delegate for event callbacks.</summary>
    public delegate void StatusChangedHandler(object sender, string status);

    /// <summary>Interface for all entities.</summary>
    public interface IEntity
    {
        string Id { get; }
        bool Validate();
    }

    /// <summary>Interface for repositories.</summary>
    public interface IRepository<T> where T : IEntity
    {
        Task<T?> FindByIdAsync(string id);
        Task<IReadOnlyList<T>> GetAllAsync();
        int Count { get; }
    }

    /// <summary>Attribute for marking cacheable methods.</summary>
    [AttributeUsage(AttributeTargets.Method)]
    public class CacheableAttribute : Attribute
    {
        public int TtlSeconds { get; }

        public CacheableAttribute(int ttlSeconds = 60)
        {
            TtlSeconds = ttlSeconds;
        }
    }

    /// <summary>Abstract base entity with shared fields.</summary>
    public abstract class Entity : IEntity
    {
        public string Id { get; }
        public DateTime CreatedAt { get; }

        protected Entity(string id)
        {
            Id = id;
            CreatedAt = DateTime.UtcNow;
        }

        public abstract bool Validate();
    }

    /// <summary>A user entity with full feature coverage.</summary>
    public class User : Entity
    {
        public string Name { get; set; }
        private readonly string _email;
        internal LogLevel Level { get; set; }
        protected bool IsActive { get; set; }

        public event StatusChangedHandler? StatusChanged;

        private static int _instanceCount = 0;

        public User(string id, string name, string email)
            : base(id)
        {
            Name = name;
            _email = email;
            Level = LogLevel.Info;
            IsActive = true;
            _instanceCount++;
        }

        public override bool Validate()
        {
            return !string.IsNullOrWhiteSpace(Name) && _email.Contains("@");
        }

        /// <summary>Fetch the user profile asynchronously.</summary>
        [Cacheable(300)]
        public async Task<Dictionary<string, object>> FetchProfileAsync()
        {
            await Task.Delay(100);
            Console.WriteLine($"Fetched profile for {Name}");
            StatusChanged?.Invoke(this, "profile_loaded");
            return new Dictionary<string, object>
            {
                ["name"] = Name,
                ["level"] = Level.ToString(),
            };
        }

        private void LogAction(string action)
        {
            Console.WriteLine($"[{Level}] {Name}: {action}");
        }

        public static int InstanceCount => _instanceCount;
    }

    /// <summary>A value type for coordinates.</summary>
    public struct Point
    {
        public double X { get; }
        public double Y { get; }

        public Point(double x, double y)
        {
            X = x;
            Y = y;
        }

        public double DistanceTo(Point other)
        {
            var dx = X - other.X;
            var dy = Y - other.Y;
            return Math.Sqrt(dx * dx + dy * dy);
        }
    }
}

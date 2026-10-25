-- Sample SQL file for extraction testing
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    email TEXT UNIQUE
);

CREATE TABLE orders (
    id SERIAL PRIMARY KEY,
    user_id INT REFERENCES users(id),
    total DECIMAL(10, 2)
);

CREATE VIEW active_users AS
SELECT id, name, email FROM users WHERE active = true;

CREATE FUNCTION calculate_tax(amount DECIMAL) RETURNS DECIMAL
BEGIN
    RETURN amount * 0.08;
END;

CREATE PROCEDURE archive_old_orders()
BEGIN
    DELETE FROM orders WHERE created_at < NOW() - INTERVAL '1 year';
END;

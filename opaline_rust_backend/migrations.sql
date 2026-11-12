CREATE TABLE IF NOT EXISTS users (
  id VARCHAR(36) PRIMARY KEY,
  email VARCHAR(255) UNIQUE NOT NULL,
  password_hash VARCHAR(255) NOT NULL,
  name VARCHAR(120) NOT NULL,
  role ENUM('client','admin') NOT NULL DEFAULT 'client',
  email_verified TINYINT(1) NOT NULL DEFAULT 0,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS plans (
  id VARCHAR(36) PRIMARY KEY,
  code VARCHAR(40) UNIQUE NOT NULL,
  name VARCHAR(120) NOT NULL,
  price DECIMAL(10,2) NOT NULL,
  currency VARCHAR(8) NOT NULL DEFAULT 'MXN',
  period VARCHAR(20) NOT NULL DEFAULT 'mensual',
  messages_per_month INT NOT NULL DEFAULT 100,
  features JSON,
  active TINYINT(1) NOT NULL DEFAULT 1,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS subscriptions (
  id VARCHAR(36) PRIMARY KEY,
  user_id VARCHAR(36) NOT NULL,
  plan_id VARCHAR(36) NOT NULL,
  status ENUM('pending','active','grace','suspended','cancelled','expired') NOT NULL DEFAULT 'pending',
  start_date DATETIME,
  end_date DATETIME,
  paypal_sub_id VARCHAR(120),
  is_current TINYINT NULL,
  current_user_id VARCHAR(36) GENERATED ALWAYS AS (CASE WHEN is_current = 1 THEN user_id ELSE NULL END) STORED,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (user_id) REFERENCES users(id),
  FOREIGN KEY (plan_id) REFERENCES plans(id),
  UNIQUE KEY uq_subscriptions_current_user (current_user_id)
);
CREATE TABLE IF NOT EXISTS payments (
  id VARCHAR(36) PRIMARY KEY,
  user_id VARCHAR(36) NOT NULL,
  plan_id VARCHAR(36) NOT NULL,
  paypal_order_id VARCHAR(120),
  amount DECIMAL(10,2) NOT NULL,
  currency VARCHAR(8) NOT NULL DEFAULT 'MXN',
  status ENUM('pending','completed','failed','refunded') NOT NULL DEFAULT 'pending',
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (user_id) REFERENCES users(id)
);
CREATE TABLE IF NOT EXISTS invoices (
  id VARCHAR(36) PRIMARY KEY,
  payment_id VARCHAR(36) NOT NULL,
  folio VARCHAR(40) NOT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (payment_id) REFERENCES payments(id)
);
CREATE TABLE IF NOT EXISTS posts (
  id VARCHAR(36) PRIMARY KEY,
  user_id VARCHAR(36) NOT NULL,
  content TEXT NOT NULL,
  rating INT,
  status ENUM('pending','approved','hidden','deleted') NOT NULL DEFAULT 'approved',
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  FOREIGN KEY (user_id) REFERENCES users(id)
);
CREATE TABLE IF NOT EXISTS reports (
  id VARCHAR(36) PRIMARY KEY,
  post_id VARCHAR(36) NOT NULL,
  user_id VARCHAR(36) NOT NULL,
  reason VARCHAR(120) NOT NULL,
  detail TEXT,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (post_id) REFERENCES posts(id)
);
CREATE TABLE IF NOT EXISTS sessions (
  id VARCHAR(36) PRIMARY KEY,
  user_id VARCHAR(36) NOT NULL,
  access_token VARCHAR(120) UNIQUE NOT NULL,
  refresh_token VARCHAR(120) UNIQUE NOT NULL,
  expires_at DATETIME NOT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (user_id) REFERENCES users(id)
);
CREATE TABLE IF NOT EXISTS expenses (
  id VARCHAR(36) PRIMARY KEY,
  date DATETIME NOT NULL,
  category VARCHAR(80) NOT NULL,
  amount DECIMAL(10,2) NOT NULL,
  currency VARCHAR(8) NOT NULL DEFAULT 'MXN',
  note TEXT,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS ai_models (
  id VARCHAR(36) PRIMARY KEY,
  code VARCHAR(120) UNIQUE NOT NULL,
  name VARCHAR(120) NOT NULL,
  enabled TINYINT(1) NOT NULL DEFAULT 1,
  is_default TINYINT(1) NOT NULL DEFAULT 0,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS usage_counters (
  id VARCHAR(36) PRIMARY KEY,
  user_id VARCHAR(36) NOT NULL,
  period VARCHAR(20) NOT NULL,
  count INT NOT NULL DEFAULT 0,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  UNIQUE KEY uq_user_period (user_id, period)
);

CREATE TABLE IF NOT EXISTS outbox_admin_actions (
  id VARCHAR(36) PRIMARY KEY,
  event_id VARCHAR(36) NOT NULL,
  admin_user_id VARCHAR(36) NOT NULL,
  action_type VARCHAR(64) NOT NULL,
  reason TEXT NOT NULL,
  previous_status VARCHAR(32) NOT NULL,
  resulting_status VARCHAR(32) NOT NULL,
  result VARCHAR(32) NOT NULL,
  idempotency_key VARCHAR(128) NULL,
  correlation_id VARCHAR(64) NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  completed_at DATETIME NULL,
  KEY idx_admin_action_event (event_id),
  KEY idx_admin_action_admin (admin_user_id),
  KEY idx_admin_action_created (created_at),
  UNIQUE KEY uq_admin_action_idempotency (admin_user_id, idempotency_key),
  FOREIGN KEY (event_id) REFERENCES outbox_events(id),
  FOREIGN KEY (admin_user_id) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS outbox_events (
  id VARCHAR(36) PRIMARY KEY,
  event_type VARCHAR(64) NOT NULL,
  aggregate_type VARCHAR(64) NOT NULL,
  aggregate_id VARCHAR(64) NOT NULL,
  payload JSON NOT NULL,
  status ENUM('pending','processing','processed','failed') NOT NULL DEFAULT 'pending',
  attempts INT NOT NULL DEFAULT 0,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  available_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  processed_at DATETIME NULL,
  last_error TEXT NULL,
  locked_at DATETIME NULL,
  claim_token VARCHAR(36) NULL,
  correlation_id VARCHAR(64) NULL,
  KEY idx_outbox_status_available (status, available_at),
  KEY idx_outbox_aggregate (aggregate_type, aggregate_id)
);

CREATE TABLE IF NOT EXISTS plan_versions (
  id VARCHAR(36) PRIMARY KEY,
  plan_id VARCHAR(36) NOT NULL,
  version INT NOT NULL,
  price DECIMAL(10,2) NOT NULL,
  currency VARCHAR(8) NOT NULL DEFAULT 'MXN',
  messages_per_month INT NOT NULL DEFAULT 100,
  requests_per_period INT NULL,
  input_tokens_per_period INT NULL,
  output_tokens_per_period INT NULL,
  total_tokens_per_period INT NULL,
  features JSON,
  effective_from DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  effective_to DATETIME NULL,
  status ENUM('draft','active','inactive') NOT NULL DEFAULT 'active',
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE KEY uq_plan_version (plan_id, version),
  FOREIGN KEY (plan_id) REFERENCES plans(id)
);

CREATE TABLE IF NOT EXISTS subscription_events (
  id VARCHAR(36) PRIMARY KEY,
  subscription_id VARCHAR(36) NOT NULL,
  event_type VARCHAR(64) NOT NULL,
  previous_status VARCHAR(32),
  new_status VARCHAR(32) NOT NULL,
  actor_type VARCHAR(32) NOT NULL DEFAULT 'system',
  actor_id VARCHAR(36),
  reason VARCHAR(240),
  correlation_id VARCHAR(64),
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (subscription_id) REFERENCES subscriptions(id)
);


CREATE TABLE IF NOT EXISTS usage_periods (
  id VARCHAR(36) PRIMARY KEY,
  user_id VARCHAR(36) NOT NULL,
  subscription_id VARCHAR(36) NOT NULL,
  plan_version_id VARCHAR(36) NOT NULL,
  period_start DATETIME NOT NULL,
  period_end DATETIME NOT NULL,
  request_limit INT NULL,
  input_token_limit INT NULL,
  output_token_limit INT NULL,
  total_token_limit INT NULL,
  requests_used INT NOT NULL DEFAULT 0,
  input_tokens_used INT NOT NULL DEFAULT 0,
  output_tokens_used INT NOT NULL DEFAULT 0,
  total_tokens_used INT NOT NULL DEFAULT 0,
  reserved_requests INT NOT NULL DEFAULT 0,
  reserved_input_tokens INT NOT NULL DEFAULT 0,
  reserved_output_tokens INT NOT NULL DEFAULT 0,
  reserved_total_tokens INT NOT NULL DEFAULT 0,
  status ENUM('open','closed') NOT NULL DEFAULT 'open',
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  UNIQUE KEY uq_usage_period_user_start (user_id, period_start),
  FOREIGN KEY (user_id) REFERENCES users(id),
  FOREIGN KEY (subscription_id) REFERENCES subscriptions(id),
  FOREIGN KEY (plan_version_id) REFERENCES plan_versions(id)
);

CREATE TABLE IF NOT EXISTS usage_reservations (
  id VARCHAR(36) PRIMARY KEY,
  user_id VARCHAR(36) NOT NULL,
  subscription_id VARCHAR(36) NOT NULL,
  usage_period_id VARCHAR(36) NOT NULL,
  request_id VARCHAR(120) NOT NULL,
  idempotency_key VARCHAR(128) NOT NULL,
  service_type VARCHAR(64) NOT NULL DEFAULT 'chat',
  model VARCHAR(120) NOT NULL,
  status ENUM('reserved','committed','released','expired','failed') NOT NULL DEFAULT 'reserved',
  estimated_input_tokens INT NOT NULL DEFAULT 0,
  estimated_output_tokens INT NOT NULL DEFAULT 0,
  estimated_total_tokens INT NOT NULL DEFAULT 0,
  actual_input_tokens INT NULL,
  actual_output_tokens INT NULL,
  actual_total_tokens INT NULL,
  token_source ENUM('provider','local','estimated','unavailable') NULL,
  measurement_status ENUM('measured','estimated','unavailable','pending') NOT NULL DEFAULT 'pending',
  response_json JSON NULL,
  recovery_owner VARCHAR(36) NULL,
  recovery_lease_expires_at DATETIME NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  expires_at DATETIME NOT NULL,
  committed_at DATETIME NULL,
  released_at DATETIME NULL,
  UNIQUE KEY uq_usage_reservation_user_idempotency (user_id, idempotency_key),
  UNIQUE KEY uq_usage_reservation_user_request (user_id, request_id),
  FOREIGN KEY (user_id) REFERENCES users(id),
  FOREIGN KEY (subscription_id) REFERENCES subscriptions(id),
  FOREIGN KEY (usage_period_id) REFERENCES usage_periods(id)
);

CREATE TABLE IF NOT EXISTS usage_records (
  id VARCHAR(36) PRIMARY KEY,
  user_id VARCHAR(36) NOT NULL,
  subscription_id VARCHAR(36) NOT NULL,
  usage_period_id VARCHAR(36) NOT NULL,
  reservation_id VARCHAR(36) NOT NULL,
  request_id VARCHAR(120) NOT NULL,
  provider VARCHAR(64) NOT NULL,
  model VARCHAR(120) NOT NULL,
  service_type VARCHAR(64) NOT NULL DEFAULT 'chat',
  input_tokens INT NOT NULL DEFAULT 0,
  output_tokens INT NOT NULL DEFAULT 0,
  total_tokens INT NOT NULL DEFAULT 0,
  token_source ENUM('provider','local','estimated','unavailable') NOT NULL,
  measurement_status ENUM('measured','estimated','unavailable','pending') NOT NULL,
  duration_ms INT NOT NULL DEFAULT 0,
  status ENUM('committed','failed') NOT NULL DEFAULT 'committed',
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE KEY uq_usage_record_user_request (user_id, request_id),
  FOREIGN KEY (user_id) REFERENCES users(id),
  FOREIGN KEY (subscription_id) REFERENCES subscriptions(id),
  FOREIGN KEY (usage_period_id) REFERENCES usage_periods(id),
  FOREIGN KEY (reservation_id) REFERENCES usage_reservations(id)
);


CREATE TABLE IF NOT EXISTS chat_operations (
  id VARCHAR(36) PRIMARY KEY,
  user_id VARCHAR(36) NOT NULL,
  subscription_id VARCHAR(36) NOT NULL,
  usage_period_id VARCHAR(36) NOT NULL,
  reservation_id VARCHAR(36) NOT NULL,
  request_id VARCHAR(120) NOT NULL,
  idempotency_key VARCHAR(128) NOT NULL,
  model_requested VARCHAR(120) NOT NULL,
  model_used VARCHAR(120) NULL,
  status ENUM('reserved','inferred','persisting','persisted','failed','manual_review') NOT NULL DEFAULT 'reserved',
  user_message TEXT NOT NULL,
  assistant_reply TEXT NULL,
  response_json JSON NULL,
  conversation_id VARCHAR(120) NULL,
  mongo_persisted TINYINT(1) NOT NULL DEFAULT 0,
  mongo_error TEXT NULL,
  token_source VARCHAR(32) NULL,
  measurement_status VARCHAR(32) NULL,
  actual_input_tokens INT NULL,
  actual_output_tokens INT NULL,
  actual_total_tokens INT NULL,
  duration_ms INT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  UNIQUE KEY uq_chat_operation_user_idempotency (user_id, idempotency_key),
  UNIQUE KEY uq_chat_operation_reservation (reservation_id),
  FOREIGN KEY (user_id) REFERENCES users(id),
  FOREIGN KEY (subscription_id) REFERENCES subscriptions(id),
  FOREIGN KEY (usage_period_id) REFERENCES usage_periods(id),
  FOREIGN KEY (reservation_id) REFERENCES usage_reservations(id)
);

CREATE TABLE IF NOT EXISTS payment_orders (
  id VARCHAR(36) PRIMARY KEY,
  user_id VARCHAR(36) NOT NULL,
  plan_id VARCHAR(36) NOT NULL,
  plan_version_id VARCHAR(36) NOT NULL,
  provider VARCHAR(32) NOT NULL DEFAULT 'paypal',
  provider_order_id VARCHAR(120) NULL,
  provider_request_id VARCHAR(120) NULL,
  idempotency_key VARCHAR(128) NOT NULL,
  amount DECIMAL(10,2) NOT NULL,
  currency VARCHAR(8) NOT NULL DEFAULT 'MXN',
  status ENUM('created','approved','captured','failed','cancelled','expired') NOT NULL DEFAULT 'created',
  description VARCHAR(240) NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  expires_at DATETIME NULL,
  captured_at DATETIME NULL,
  activated_subscription_id VARCHAR(36) NULL,
  activated_at DATETIME NULL,
  UNIQUE KEY uq_payment_orders_provider_order (provider, provider_order_id),
  UNIQUE KEY uq_payment_orders_user_idempotency (user_id, idempotency_key),
  UNIQUE KEY uq_payment_orders_provider_request (provider_request_id),
  FOREIGN KEY (user_id) REFERENCES users(id),
  FOREIGN KEY (plan_id) REFERENCES plans(id),
  FOREIGN KEY (plan_version_id) REFERENCES plan_versions(id)
);

CREATE TABLE IF NOT EXISTS payment_transactions (
  id VARCHAR(36) PRIMARY KEY,
  payment_order_id VARCHAR(36) NOT NULL,
  user_id VARCHAR(36) NOT NULL,
  provider VARCHAR(32) NOT NULL DEFAULT 'paypal',
  provider_transaction_id VARCHAR(120) NOT NULL,
  amount DECIMAL(10,2) NOT NULL,
  currency VARCHAR(8) NOT NULL DEFAULT 'MXN',
  status ENUM('pending','completed','denied','refunded','partially_refunded','reversed') NOT NULL DEFAULT 'pending',
  subscription_id VARCHAR(36) NULL,
  captured_at DATETIME NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  UNIQUE KEY uq_payment_transactions_provider_transaction (provider_transaction_id),
  FOREIGN KEY (payment_order_id) REFERENCES payment_orders(id),
  FOREIGN KEY (user_id) REFERENCES users(id),
  FOREIGN KEY (subscription_id) REFERENCES subscriptions(id)
);

CREATE TABLE IF NOT EXISTS payment_webhook_events (
  id VARCHAR(36) PRIMARY KEY,
  provider VARCHAR(32) NOT NULL DEFAULT 'paypal',
  provider_event_id VARCHAR(120) NOT NULL,
  event_type VARCHAR(120) NOT NULL,
  resource_id VARCHAR(120) NULL,
  payload JSON NULL,
  payload_hash VARCHAR(128) NOT NULL,
  processing_status ENUM('received','pending','processed','ignored','failed') NOT NULL DEFAULT 'received',
  received_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  processed_at DATETIME NULL,
  last_error TEXT NULL,
  retry_count INT NOT NULL DEFAULT 0,
  UNIQUE KEY uq_payment_webhook_provider_event (provider, provider_event_id)
);

CREATE TABLE IF NOT EXISTS payment_refunds (
  id VARCHAR(36) PRIMARY KEY,
  payment_transaction_id VARCHAR(36) NOT NULL,
  user_id VARCHAR(36) NOT NULL,
  provider_refund_id VARCHAR(120) NULL,
  idempotency_key VARCHAR(128) NOT NULL,
  amount DECIMAL(10,2) NOT NULL,
  currency VARCHAR(8) NOT NULL DEFAULT 'MXN',
  status ENUM('requested','pending','completed','failed','cancelled') NOT NULL DEFAULT 'requested',
  reason VARCHAR(240) NOT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  completed_at DATETIME NULL,
  UNIQUE KEY uq_payment_refunds_user_idempotency (user_id, idempotency_key),
  UNIQUE KEY uq_payment_refunds_provider_refund (provider_refund_id),
  FOREIGN KEY (payment_transaction_id) REFERENCES payment_transactions(id),
  FOREIGN KEY (user_id) REFERENCES users(id)
);

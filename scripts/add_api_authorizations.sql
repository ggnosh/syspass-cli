-- Add API authorizations for the sysPass admin user.
--
-- The API checks the token against the action of each request
-- (ApiService::setup -> AuthTokenService::getTokenByToken), so every action the CLI uses
-- needs its own authorization row. These are the 29 actions the API controllers reference,
-- which is also the set the web UI token form offers; 26 of them are inserted here and
-- the three secured ones must come from the UI (see below).
--
-- Run (token taken from resources/test_config.json):
--   docker exec -i syspass-db mariadb -u root -p"$MARIADB_ROOT_PASSWORD" syspass \
--     < <(echo "SET @token = '$(jq -r .token resources/test_config.json)';"; cat scripts/add_api_authorizations.sql)
--
-- Secured actions (AuthTokenService::SECURED_ACTIONS) are not inserted: their rows carry an
-- encrypted vault and a hash key built from the password you give when creating the
-- authorization, which SQL cannot produce. A row without them makes the action look granted
-- but ApiService::getMasterPassFromVault() fails the hash check and returns 403. Create these
-- in the UI (Administration -> Authorizations), using the same password as in
-- resources/test_config.json:
--   4    New Account            ACCOUNT_CREATE
--   7    View password          ACCOUNT_VIEW_PASS
--   8    Edit Account Password  ACCOUNT_EDIT_PASS
--
-- Notes:
--   uk_AuthToken_01 (token, actionId) is unique, so INSERT IGNORE is idempotent and
--   re-running the script only adds what is missing.

SET @login = 'admin';
SET @user_id = (SELECT id FROM User WHERE login = @login LIMIT 1);

-- Token for all rows: pass it in from resources/test_config.json (see DEVELOPMENT.md),
-- otherwise reuse the token of an existing authorization for this user.
SET @token = COALESCE(@token, (SELECT token FROM AuthToken WHERE userId = @user_id LIMIT 1));

CREATE TEMPORARY TABLE api_actions (actionId INT PRIMARY KEY, name VARCHAR(32));

INSERT INTO api_actions (actionId, name) VALUES
-- ACCOUNT
  (2, 'ACCOUNT_SEARCH'),
  (3, 'ACCOUNT_VIEW'),
  (5, 'ACCOUNT_EDIT'),
  (6, 'ACCOUNT_DELETE'),
-- CATEGORY
  (102, 'CATEGORY_SEARCH'),
  (103, 'CATEGORY_VIEW'),
  (104, 'CATEGORY_CREATE'),
  (105, 'CATEGORY_EDIT'),
  (106, 'CATEGORY_DELETE'),
-- CLIENT
  (302, 'CLIENT_SEARCH'),
  (303, 'CLIENT_VIEW'),
  (304, 'CLIENT_CREATE'),
  (305, 'CLIENT_EDIT'),
  (306, 'CLIENT_DELETE'),
-- TAG
  (202, 'TAG_SEARCH'),
  (203, 'TAG_VIEW'),
  (204, 'TAG_CREATE'),
  (205, 'TAG_EDIT'),
  (206, 'TAG_DELETE'),
-- GROUP
  (802, 'GROUP_SEARCH'),
  (803, 'GROUP_VIEW'),
  (804, 'GROUP_CREATE'),
  (805, 'GROUP_EDIT'),
  (806, 'GROUP_DELETE'),
-- CONFIG
  (1541, 'CONFIG_BACKUP_RUN'),
  (1561, 'CONFIG_EXPORT_RUN');

INSERT IGNORE INTO AuthToken (userId, actionId, token, createdBy, startDate, vault, hash)
SELECT @user_id, a.actionId, @token, @user_id, UNIX_TIMESTAMP(), NULL, NULL
FROM api_actions a;

-- What the admin user has after this
SELECT a.actionId, a.name, t.id AS row_id
FROM api_actions a
LEFT JOIN AuthToken t ON t.actionId = a.actionId AND t.token = @token
ORDER BY a.actionId;

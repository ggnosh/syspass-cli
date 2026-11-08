-- Add API authorizations for the sysPass 2 admin user.
--
-- The v2 API resolves the action from the method name (SyspassApi::getActions) and looks up
-- the token per action (ApiToken::getTokenByToken), so each action needs its own row in
-- authTokens. Those are the 8 actions the v2 API implements, which is also the set the web UI
-- token form offers; 6 of them are inserted here and the two secured ones must come from the
-- UI (see below).
--
-- Run (token taken from resources/test_config_v2.json):
--   docker exec -i syspass-dbv2 mariadb -u root -p"$MARIADB_ROOT_PASSWORD" syspass \
--     < <(echo "SET @token = '$(jq -r .token resources/test_config_v2.json)';"; cat scripts/add_api_authorizations_v2.sql)
--
-- Secured actions (ApiBase::passIsNeeded -> ACTION_ACC_NEW, ACTION_ACC_VIEW_PASS) are not
-- inserted: their rows carry an encrypted vault and a hash key built from the password you give
-- when creating the authorization, which SQL cannot produce. A row without them makes the action
-- look granted but ApiBase::doAuth() fails the hash check and returns 403. Create these in the
-- UI (Authorizations page), using the same password as in resources/test_config_v2.json:
--   101  New Account        ACTION_ACC_NEW
--   104  View password      ACTION_ACC_VIEW_PASS
--
-- Notes:
--   v2 has no unique key on (token, actionId) - only IDX_checkToken - so the insert skips rows
--   that already exist instead of relying on INSERT IGNORE. Re-running is still safe.

SET @login = 'admin';
SET @user_id = (SELECT user_id FROM usrData WHERE user_login = @login LIMIT 1);

-- Token for all rows: pass it in from resources/test_config_v2.json (see DEVELOPMENT.md),
-- otherwise reuse the token of an existing authorization for this user.
SET @token = COALESCE(@token, (SELECT authtoken_token FROM authTokens WHERE authtoken_userId = @user_id LIMIT 1));

CREATE TEMPORARY TABLE api_actions (actionId INT PRIMARY KEY, name VARCHAR(32));

INSERT INTO api_actions (actionId, name) VALUES
-- ACCOUNT
  (1, 'ACTION_ACC_SEARCH'),
  (100, 'ACTION_ACC_VIEW'),
  (103, 'ACTION_ACC_DELETE'),
-- MANAGEMENT
  (61, 'ACTION_MGM_CATEGORIES'),
  (62, 'ACTION_MGM_CUSTOMERS'),
-- CONFIG
  (804, 'ACTION_CFG_BACKUP');

INSERT INTO authTokens (authtoken_userId, authtoken_actionId, authtoken_token, authtoken_createdBy, authtoken_startDate, authtoken_vault, authtoken_hash)
SELECT @user_id, a.actionId, @token, @user_id, UNIX_TIMESTAMP(), NULL, NULL
FROM api_actions a
WHERE NOT EXISTS (
  SELECT 1 FROM authTokens t
  WHERE t.authtoken_actionId = a.actionId AND t.authtoken_token = @token
);

-- What the admin user has after this
SELECT a.actionId, a.name, t.authtoken_id AS row_id
FROM api_actions a
LEFT JOIN authTokens t ON t.authtoken_actionId = a.actionId AND t.authtoken_token = @token
ORDER BY a.actionId;

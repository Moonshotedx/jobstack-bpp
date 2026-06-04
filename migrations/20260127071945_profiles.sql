CREATE TABLE profiles (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  profile_id TEXT NOT NULL,
  beckn_structure JSONB,
  metadata JSONB,
  hash TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  last_synced_at TIMESTAMPTZ,
  transaction_id TEXT NOT NULL,
  bpp_id TEXT NOT NULL,
  bpp_uri TEXT NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX idx_profiles_profile_id
  ON profiles(profile_id);


CREATE INDEX idx_profiles_metadata_gin
  ON profiles USING GIN (metadata);

CREATE INDEX idx_profiles_beckn_structure_gin
  ON profiles USING GIN (beckn_structure);

CREATE UNIQUE INDEX idx_profiles_hash_unique
  ON profiles(hash);

CREATE INDEX idx_profiles_last_synced_at_not_null
ON profiles (last_synced_at)
WHERE last_synced_at IS NOT NULL;

CREATE INDEX idx_profiles_bpp_txn
ON profiles (bpp_id, transaction_id);

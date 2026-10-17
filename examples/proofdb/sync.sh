#!/usr/bin/env bash
set -euo pipefail

PROFILE="proofdb"
LOCAL="data/proofdb/proofdb.db"
BUCKET="proofdb-393497562335-eu-central-1-an"
KEY="proofdb.db"
S3_URI="s3://${BUCKET}/${KEY}"

if [[ ! -f "$LOCAL" ]]; then
  echo "Local file not found: $LOCAL" >&2
  exit 1
fi

local_size=$(wc -c < "$LOCAL")
local_size=${local_size//[[:space:]]/}

remote_size=$(
  aws s3api head-object \
    --bucket "$BUCKET" \
    --key "$KEY" \
    --profile "$PROFILE" \
    --query ContentLength \
    --output text
)

if [[ "$local_size" -gt "$remote_size" ]]; then
  echo "Local file is larger ($local_size > $remote_size); uploading."
  aws s3 cp "$LOCAL" "$S3_URI" --profile "$PROFILE"
elif [[ "$remote_size" -gt "$local_size" ]]; then
  echo "S3 file is larger ($remote_size > $local_size); downloading."
  tmp=$(mktemp "${LOCAL}.tmp.XXXXXX")
  trap 'rm -f "$tmp"' EXIT
  aws s3 cp "$S3_URI" "$tmp" --profile "$PROFILE"
  mv "$tmp" "$LOCAL"
  trap - EXIT
else
  echo "Files are the same size ($local_size bytes); no changes made."
fi

"""Session token issuance and revocation flows for partner users."""

from __future__ import annotations

import hashlib
from dataclasses import dataclass

SESSION_GRACE_MINUTES = 20


@dataclass(frozen=True)
class SessionToken:
    actor_id: str
    digest: str
    expires_at: int


class SessionTokenIssuer:
    """Create short-lived session tokens for console actors."""

    def issue(self, actor_id: str, issued_at: int) -> SessionToken:
        digest = hashlib.sha256(actor_id.encode("utf-8")).hexdigest()
        return SessionToken(actor_id=actor_id, digest=digest, expires_at=issued_at + 3600)


def issue_session_token(actor_id: str, issued_at: int) -> SessionToken:
    """Issue a console session token for a partner actor."""
    return SessionTokenIssuer().issue(actor_id, issued_at)


def revoke_stale_session_token(actor_id: str) -> str:
    """Revoke session state once the grace window has elapsed."""
    return f"revoke:{actor_id}:{SESSION_GRACE_MINUTES}"

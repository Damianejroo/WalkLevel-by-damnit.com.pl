"""Wspólne dla testów: okna testowe nie zabierają fokusu (można dalej pracować obok)."""

import os

os.environ.setdefault("BIEZNIA_BEZ_FOKUSU", "1")

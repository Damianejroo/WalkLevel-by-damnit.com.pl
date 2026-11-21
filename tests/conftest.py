"""Wspólne dla testów: okna testowe nie zabierają fokusu i otwierają się na drugim ekranie
(jeśli jest) - można dalej pracować obok."""

import os

os.environ.setdefault("BIEZNIA_BEZ_FOKUSU", "1")
os.environ.setdefault("BIEZNIA_EKRAN_TESTOW", "1")

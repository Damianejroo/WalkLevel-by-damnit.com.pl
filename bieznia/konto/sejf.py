"""Token konta w Menedżerze poświadczeń Windows (chroniony przez DPAPI dla tego konta Windows).

Nigdy w plikach z danymi, w kopii danych ani w logach. Hasła nie zapisujemy wcale - tylko
token, który serwer może unieważnić (zmiana hasła = wylogowanie wszędzie). Poza Windows
(testy) token żyje tylko w pamięci.
"""

from __future__ import annotations

import ctypes
import logging
import sys
from ctypes import wintypes
from typing import Protocol

log = logging.getLogger("bieznia")

TARGET = "Bieznia/konto"
CRED_TYPE_GENERIC = 1
CRED_PERSIST_LOCAL_MACHINE = 2
MAX_BLOB = 2560


class TokenStore(Protocol):
    def get(self) -> str: ...

    def set(self, token: str) -> None: ...

    def clear(self) -> None: ...


class MemoryStore:
    def __init__(self) -> None:
        self.token = ""

    def get(self) -> str:
        return self.token

    def set(self, token: str) -> None:
        self.token = token

    def clear(self) -> None:
        self.token = ""


class _Credential(ctypes.Structure):
    _fields_ = [
        ("Flags", wintypes.DWORD),
        ("Type", wintypes.DWORD),
        ("TargetName", wintypes.LPWSTR),
        ("Comment", wintypes.LPWSTR),
        ("LastWritten", wintypes.FILETIME),
        ("CredentialBlobSize", wintypes.DWORD),
        ("CredentialBlob", ctypes.POINTER(ctypes.c_ubyte)),
        ("Persist", wintypes.DWORD),
        ("AttributeCount", wintypes.DWORD),
        ("Attributes", ctypes.c_void_p),
        ("TargetAlias", wintypes.LPWSTR),
        ("UserName", wintypes.LPWSTR),
    ]


class WindowsStore:
    """Menedżer poświadczeń Windows (CredWriteW / CredReadW / CredDeleteW)."""

    def __init__(self, target: str = TARGET) -> None:
        self.target = target
        advapi = ctypes.WinDLL("advapi32", use_last_error=True)
        self._write = advapi.CredWriteW
        self._write.argtypes = [ctypes.POINTER(_Credential), wintypes.DWORD]
        self._write.restype = wintypes.BOOL
        self._read = advapi.CredReadW
        self._read.argtypes = [
            wintypes.LPCWSTR,
            wintypes.DWORD,
            wintypes.DWORD,
            ctypes.POINTER(ctypes.POINTER(_Credential)),
        ]
        self._read.restype = wintypes.BOOL
        self._delete = advapi.CredDeleteW
        self._delete.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD]
        self._delete.restype = wintypes.BOOL
        self._free = advapi.CredFree
        self._free.argtypes = [ctypes.c_void_p]

    def get(self) -> str:
        found = ctypes.POINTER(_Credential)()
        if not self._read(self.target, CRED_TYPE_GENERIC, 0, ctypes.byref(found)):
            return ""
        try:
            cred = found.contents
            blob = ctypes.string_at(cred.CredentialBlob, cred.CredentialBlobSize)
            return blob.decode("utf-8", "replace")
        finally:
            self._free(found)

    def set(self, token: str) -> None:
        data = token.encode("utf-8")
        if len(data) > MAX_BLOB:
            raise ValueError("token za długi dla Menedżera poświadczeń")
        blob = (ctypes.c_ubyte * len(data)).from_buffer_copy(data)
        cred = _Credential()
        cred.Type = CRED_TYPE_GENERIC
        cred.TargetName = self.target
        cred.Comment = "WalkLevel - logowanie do konta online"
        cred.CredentialBlobSize = len(data)
        cred.CredentialBlob = ctypes.cast(blob, ctypes.POINTER(ctypes.c_ubyte))
        cred.Persist = CRED_PERSIST_LOCAL_MACHINE
        cred.UserName = "bieznia"
        if not self._write(ctypes.byref(cred), 0):
            raise OSError(ctypes.get_last_error(), "CredWriteW")

    def clear(self) -> None:
        self._delete(self.target, CRED_TYPE_GENERIC, 0)


def default_store(target: str = TARGET) -> TokenStore:
    if sys.platform == "win32":
        try:
            return WindowsStore(target)
        except (OSError, AttributeError):
            log.warning("Menedżer poświadczeń niedostępny - logowanie tylko do zamknięcia")
    return MemoryStore()

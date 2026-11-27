*This is an English translation for convenience. In case of any discrepancy, the Polish version
([PRIVACY.pl.md](PRIVACY.pl.md)) prevails.*

# WalkLevel app privacy policy

English · [Polski](PRIVACY.pl.md)

The data controller is me, Damian Skrzypiec - I run damnit.com.pl privately.
For anything about your data, write to kontakt@damnit.com.pl.

## In short

WalkLevel runs on your computer. It has no ads and no analytics. Treadmill data, settings,
step history, level, achievements and logs stay with you. The online account is optional:
without it nothing leaves the computer except the update check. Something reaches me only
when you create an online account or send a problem report yourself.

## What stays on your computer

In the `%LOCALAPPDATA%\Bieznia` folder:

- people and game profiles, app settings (speed thresholds, theme, the remembered treadmill -
  its name and Bluetooth address),
- step history: walking sessions (time, steps, distance, treadmill model),
- with an online account: the account's nickname and email and which sessions have already
  been sent; the login token itself is kept in Windows Credential Manager (only with
  "Remember login"),
- event log: app start, errors, the course of the connection with the treadmill,
- detailed Bluetooth logs, if turned on (Settings > Log and reports): the raw conversation
  with the treadmill, including speed and step count,
- the diagnostic report, if you save it: a list of nearby Bluetooth devices (names and
  addresses) and the conversation with the treadmill,
- recordings of walks, if you turn recording on.

The app does not record pressed keys (it only checks the control keys), passwords or anything
from games. You delete everything by deleting this folder (you remove the account token by
logging out).

## When the app connects to the internet

**Checking for and downloading updates.** On start and after clicking Update, the app asks
GitHub for the latest version, and after clicking Update it downloads the installer from
GitHub. GitHub then sees your computer's IP address and the app version. I get no data from
this. Purpose: delivering fixes. Legal basis: my legitimate interest - keeping the app working
and secure (Article 6(1)(f) GDPR).

**Treadmill model profiles.** After clicking "Check for new models" (Settings > Supported
treadmills), the app downloads the list and files of treadmill profiles from GitHub (data
only: speed ranges, model names). GitHub then sees your computer's IP address. Legal basis as
for updates.

**Online account (optional).** When you create an account or log in, the app connects to the
bieznia.damnit-live.pl server and sends the account data and the walking sessions of the
person linked to the account. What exactly, why, who sees it and how long it is kept is
described in the [Online account terms and privacy](https://bieznia.damnit-live.pl/bieznia).
Without an account the app does not connect to this server.

**Problem report.** When you click Send report (or email me the prepared package), I receive:
your description, contact (if you give one), app and Bluetooth logs, the latest diagnostic
report, app settings, and the app and Windows versions. The report window shows this before
sending. From logs and settings the app removes paths containing the Windows user name, e-mail
addresses, names of people and game profiles, pseudonyms, account ids and the addresses of
Bluetooth devices other than the selected treadmill. The report goes through the
bieznia.damnit-live.pl server (OVHcloud) straight to my inbox kontakt@damnit.com.pl; the server
does not keep the content or the package, only the report number, time, app version and size,
for 30 days (protection against abuse). You can also save the package to disk and send it
yourself. Purpose: finding and fixing the bug. Legal basis: my legitimate interest - helping
to solve the reported problem (Article 6(1)(f) GDPR). I keep a report until the problem is
solved and then delete it.

## Who I share data with

- **GitHub, Inc.** - handles checking for and downloading updates and treadmill profiles,
  under its own
  [privacy statement](https://docs.github.com/site-policy/privacy-policies/github-general-privacy-statement).
- Online account: a server at OVHcloud (Poland) and encrypted backups - details in the
  [account terms](https://bieznia.damnit-live.pl/bieznia).

I do not share reports with anyone.

## Your rights

You can ask for access to your data, its correction, deletion, restriction or portability.
You can also object to what I do on the basis of legitimate interest. Just write to
kontakt@damnit.com.pl.

If you believe I process your data unlawfully, you can lodge a complaint with the President
of the Personal Data Protection Office (Prezes Urzędu Ochrony Danych Osobowych, ul. Stawki 2,
00-193 Warsaw, Poland).

## Changes

If the app starts sending any other data or using a tool from another company, I will
describe it here before turning it on.

Effective from: 2026-11-25 (added: online account, treadmill model profiles)

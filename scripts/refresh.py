import json
import os
import subprocess
import sys
import unicodedata
import urllib.error
import urllib.request

# football-data.org v4. FIFA World Cup competition code = WC (free tier includes it).
API_URL = "https://api.football-data.org/v4/competitions/WC/matches"
SEASON = os.environ.get("WC_SEASON", "")  # optional ?season=YYYY (start year); empty = current edition
BINARY = os.environ.get("WM_BINARY", "./target/release/wm2026")
RESULTS_PATH = os.environ.get("WM_RESULTS", "data/results.json")

# our team code -> accepted names (matched accent/case-insensitively; tla is tried first)
ALIASES = {
    "MEX": ["mexico"], "RSA": ["south africa"], "KOR": ["south korea", "korea republic", "korea"],
    "CZE": ["czechia", "czech republic"], "CAN": ["canada"],
    "BIH": ["bosnia and herzegovina", "bosnia herzegovina", "bosnia"], "QAT": ["qatar"],
    "SUI": ["switzerland"], "BRA": ["brazil"], "MAR": ["morocco"], "HAI": ["haiti"],
    "SCO": ["scotland"], "USA": ["united states", "usa", "united states of america"],
    "PAR": ["paraguay"], "AUS": ["australia"], "TUR": ["turkiye", "turkey"], "GER": ["germany"],
    "CUW": ["curacao"], "CIV": ["ivory coast", "cote divoire", "cote d ivoire"], "ECU": ["ecuador"],
    "NED": ["netherlands"], "JPN": ["japan"], "SWE": ["sweden"], "TUN": ["tunisia"],
    "BEL": ["belgium"], "EGY": ["egypt"], "IRN": ["iran", "ir iran"], "NZL": ["new zealand"],
    "ESP": ["spain"], "CPV": ["cape verde", "cabo verde", "cape verde islands"], "KSA": ["saudi arabia"],
    "URU": ["uruguay"], "FRA": ["france"], "SEN": ["senegal"], "IRQ": ["iraq"], "NOR": ["norway"],
    "ARG": ["argentina"], "ALG": ["algeria"], "AUT": ["austria"], "JOR": ["jordan"],
    "POR": ["portugal"], "COD": ["dr congo", "congo dr", "democratic republic of congo"],
    "UZB": ["uzbekistan"], "COL": ["colombia"], "ENG": ["england"], "CRO": ["croatia"],
    "GHA": ["ghana"], "PAN": ["panama"],
}


def load_dotenv(path: str = ".env") -> None:
    """Load KEY=VALUE lines from a local .env (already-set env vars win, e.g. CI secrets)."""
    try:
        for line in open(path):
            line = line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            k, v = line.split("=", 1)
            os.environ.setdefault(k.strip(), v.strip().strip("'\""))
    except FileNotFoundError:
        pass


def norm(s: str) -> str:
    s = unicodedata.normalize("NFKD", s or "").encode("ascii", "ignore").decode()
    return "".join(c for c in s.lower() if c.isalnum() or c == " ").strip()


NAME_TO_CODE = {norm(name): code for code, names in ALIASES.items() for name in names}


def team_code(team: dict) -> str | None:
    tla = (team.get("tla") or "").upper()
    if tla in ALIASES:
        return tla
    return NAME_TO_CODE.get(norm(team.get("name") or team.get("shortName") or ""))


def load_schedule() -> dict:
    """frozenset({home_code, away_code}) -> (match_id, home_code)."""
    out = subprocess.run([BINARY, "--dump-schedule"], capture_output=True, text=True, check=True)
    sched = {}
    for fx in json.loads(out.stdout):
        sched[frozenset((fx["home"], fx["away"]))] = (fx["id"], fx["home"])
    return sched


def fetch_matches(key: str) -> list:
    url = API_URL + (f"?season={SEASON}" if SEASON else "")
    req = urllib.request.Request(url, headers={"X-Auth-Token": key})
    with urllib.request.urlopen(req, timeout=30) as resp:
        return json.load(resp).get("matches", [])


def main() -> int:
    load_dotenv()
    key = os.environ.get("FOOTBALL_DATA_KEY")
    if not key:
        print("FOOTBALL_DATA_KEY not set — keeping committed results.")
        return 0
    try:
        matches = fetch_matches(key)
    except urllib.error.HTTPError as e:
        print(f"API HTTP {e.code} — keeping committed results.")
        return 0
    except Exception as e:  # network/parse issues must not break the daily deploy
        print(f"API fetch failed ({e}) — keeping committed results.")
        return 0

    schedule = load_schedule()
    existing = json.loads(open(RESULTS_PATH).read())
    by_id = {r["match"]: r for r in existing.get("results", [])}

    changed, unmapped = 0, []
    for m in matches:
        if m.get("status") != "FINISHED":
            continue
        hc, ac = team_code(m.get("homeTeam", {})), team_code(m.get("awayTeam", {}))
        if not hc or not ac:
            unmapped.append(f"{m.get('homeTeam', {}).get('name')} vs {m.get('awayTeam', {}).get('name')}")
            continue
        fx = schedule.get(frozenset((hc, ac)))
        ft = m.get("score", {}).get("fullTime", {})
        if not fx or ft.get("home") is None or ft.get("away") is None:
            continue
        mid, fixture_home = fx
        # store in our fixture's orientation (home = the team our id calls home)
        if hc == fixture_home:
            hg, ag, h_is_fix_home = ft["home"], ft["away"], True
        else:
            hg, ag, h_is_fix_home = ft["away"], ft["home"], False
        rec = {"match": mid, "home": int(hg), "away": int(ag)}
        if mid.startswith("M"):
            duration = m.get("score", {}).get("duration")  # REGULAR_TIME / EXTRA_TIME / PENALTY_SHOOTOUT
            rec["decided"] = {"EXTRA_TIME": "aet", "PENALTY_SHOOTOUT": "pens"}.get(duration, "90")
            if hg == ag:
                winner = m.get("score", {}).get("winner")  # HOME_TEAM / AWAY_TEAM (advancer)
                if winner == "HOME_TEAM":
                    rec["winner"] = "home" if h_is_fix_home else "away"
                elif winner == "AWAY_TEAM":
                    rec["winner"] = "away" if h_is_fix_home else "home"
        if by_id.get(mid) != rec:
            changed += 1
        by_id[mid] = rec

    existing["results"] = sorted(by_id.values(), key=lambda r: r["match"])
    open(RESULTS_PATH, "w").write(json.dumps(existing, indent=2) + "\n")
    if unmapped:
        print(f"unmapped ({len(unmapped)}): {unmapped[:5]}")
    print(f"refresh done — {len(by_id)} results on file, {changed} added/changed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())

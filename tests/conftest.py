import os

# never pop the first-run wizard over windows the tests open
os.environ.setdefault("HARBOR_NO_WELCOME", "1")

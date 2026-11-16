import json
import requests
import subprocess
import sys
import time

from commons import SET_VERSION_SCRIPT, PACKAGE_JSON_PATH, CARGO_TOML_FILE, PROJ_DIR, SRC_DIR, SRC_TAURI_SRC_DIR, PKGBUILD_PATH
from extract_dependencies import do_extract

if __name__ == "__main__":
    if subprocess.check_output(["git", "branch", "--show-current"], text=True).strip() != "main":
        print("ERROR: Releases are only allowed in main branch")
        sys.exit(1)

    if len(subprocess.check_output(["git", "status", "--porcelain"], text=True).strip().splitlines()) > 0:
        print("ERROR: The repository has uncommited changes")
        sys.exit(1)

    todos = {}
    for file in [*SRC_TAURI_SRC_DIR.rglob("*.rs"), *SRC_DIR.rglob("*.ts"), *SRC_DIR.rglob("*.tsx")]:
        with open(file, "r", encoding="utf-8") as f:
            local_todo = []
            for i, line in enumerate(f, start=1):
                if "TODO" in line:
                    local_todo.append((i, line.rstrip()))
            if len(local_todo) > 0:
                todos[str(file).replace(str(PROJ_DIR),"")[1:]]=local_todo

    if todos:
        print("TODOs not allowed in release, found:")
        for file,todos in todos.items():
            print(f"    {file}")
            for line_num, line_content in todos:
                print(f"        Line {str(line_num).ljust(4, " ")} -> {line_content}")
            print("")
        sys.exit(1)

    subprocess.check_call(["make", "test"])
    subprocess.check_call(["make", "build"])

    subprocess.check_call(["pnpm", "tsc"])
    subprocess.check_call(["cargo", "clippy", "--release", "--manifest-path", CARGO_TOML_FILE, "--", "-D", "warnings"])
    subprocess.check_call(["pnpm", "prettier", "--write", SRC_DIR])
    subprocess.check_call(["cargo", "fmt"], cwd=SRC_TAURI_SRC_DIR)

    if len(subprocess.check_output(["git", "status", "--porcelain"], text=True).strip().splitlines()) > 0:
        subprocess.check_call(["git", "commit", "-am", "[chore] Check and format before release"])

    dependencies = do_extract()
    with open(PKGBUILD_PATH, "r", encoding="utf-8") as f:
        lines = f.readlines()
    for idx,line in enumerate(lines):
        if line.startswith("depends=("):
            deps_str = ""
            for dep in dependencies:
                deps_str = deps_str + f"'{dep}' "
            if deps_str.endswith(" "):
                deps_str = deps_str[0:-1]
            lines[idx] = f"depends=({deps_str})\n"
    with open(PKGBUILD_PATH, "w", encoding="utf-8") as f:
        f.write("".join(lines))

    if len(subprocess.check_output(["git", "status", "--porcelain"], text=True).strip().splitlines()) > 0:
        subprocess.check_call(["git", "commit", "-am", "[chore] Update PKGBUILD"])

    subprocess.check_call(["python", SET_VERSION_SCRIPT])

    with open(PACKAGE_JSON_PATH, "r", encoding="utf-8") as f:
        version = json.load(f)["version"]

    subprocess.check_call(["git", "commit", "-am", f"[release] {version}"])
    subprocess.check_call(["git", "push"])

    print("Waiting for github release to be published...")
    t0 = time.time()
    URL = f"https://api.github.com/repos/Emiliopg91/strength-coach-for-garmin/releases/tags/{version}"
    while True:
        try:
            r = requests.get(
                URL,
                timeout=3,
            )

            if r.status_code == 200:
                print(f"✅ {version} published after {(int(time.time()-t0)*1000)/1000}s in https://github.com/Emiliopg91/strength-coach-for-garmin/releases/tag/{version}")
                break

        except requests.RequestException as e:
            continue

        time.sleep(3)

    print("Waiting for AUR to be published...")
    t0 = time.time()
    URL = "https://aur.archlinux.org/rpc/?v=5&type=info&arg=strength-coach-for-garmin"
    while True:
        try:
            r = requests.get(
                URL,
                timeout=3,
            )

            if r.status_code == 200:
                results = r.json()["results"]
                # Empty until the first release of the package reaches AUR
                if results and results[0]["Version"].startswith(f"{version}-1"):
                    print(f"✅ {version} published after {(int(time.time()-t0)*1000)/1000}s!")
                    break

        except requests.RequestException as e:
            continue

        time.sleep(3)

    subprocess.check_call(["git", "pull"])

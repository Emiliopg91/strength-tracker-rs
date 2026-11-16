import json
import os
import re
import shutil
import subprocess
import yaml

from commons import DIST_DIR, PACKAGE_JSON_PATH, PKGBUILD_PATH, PKGBUILD_DIST_PATH, PREVIOUS_VERSIONS_FILE, CURRENT_VERSIONS_FILE, CHANGELOG_MD_FILE, INSTALL_DIST_FILE, INSTALL_PATH


def generate_install():
    print("Generating install file...")
    shutil.copy2(INSTALL_PATH, INSTALL_DIST_FILE)


def generate_srcinfo():
    print("Generating .SRCINFO...")
    os.chmod(DIST_DIR, 0o777)
    subprocess.run(
        [
            "docker",
            "run",
            "--rm",
            "-u",
            f"{os.getuid()}:{os.getgid()}",
            "-v",
            f"{DIST_DIR}:/pkg",
            "epulidogil/rog-perf-tuner-srcinfo:latest",
        ],
        check=True,
    )


def generate_pkgbuild():
    print("Generating PKGBUILD...")

    shutil.copy2(PKGBUILD_PATH, PKGBUILD_DIST_PATH)

    with open(PACKAGE_JSON_PATH, "r", encoding="utf-8") as f:
        package_json = json.load(f)

    with open(PKGBUILD_DIST_PATH, "r", encoding="utf-8") as f:
        content = f.read()

    content = content.replace("pkgver=", f"pkgver={package_json["version"]}")

    with open(PKGBUILD_DIST_PATH, "w", encoding="utf-8") as f:
        f.write(content)


def create_dist_dir():
    if os.path.exists(DIST_DIR):
        print("Cleaning old dist folder...")
        shutil.rmtree(DIST_DIR)
    print("Creating dist folder...")
    os.makedirs(DIST_DIR)



def generate_changelog():
    print("Generating changelog...")
    TAG_REGEX = re.compile(r"^\d+\.\d+\.\d+(-\d)?+$")

    def run_git(cmd):
        return subprocess.check_output(cmd, text=True).strip()

    def get_previous_version_tag():
        tags = run_git(["git", "tag", "--sort=-creatordate"]).splitlines()

        matched_tags = [tag for tag in tags if TAG_REGEX.match(tag)]

        if len(matched_tags) > 0:
            return matched_tags[0]
        return None

    def get_commits_since_tag(tag):
        if not tag:
            cmd = ["git", "log", "--pretty=format:%H %s"]
        else:
            cmd = ["git", "log", f"{tag}..HEAD", "--pretty=format:%H----%s"]
        print(f"Getting commits since {tag}")
        return run_git(cmd).splitlines()

    tag = get_previous_version_tag()
    commits = get_commits_since_tag(tag)
    commits.reverse()

    entries = {"feature": [], "improve": [], "fix": []}

    for c in commits:
        commit_hash, msg = c.split("----")
        msg = msg.replace("[ci skip]", "").strip()

        for typeEntry, typeEntries in entries.items():
            if msg.startswith(f"[{typeEntry}]"):
                typeEntries.append(
                    f'<td><a href="https://github.com/Emiliopg91/strength-coach-for-garmin/commit/{commit_hash}">{commit_hash[0:7]}</a></td><td>{msg.replace(f"[{typeEntry}]", "").strip().capitalize()}</td>'
                )

    lines: list[str] = ["# Changes for release"]

    if (
        len(entries.get("feature")) > 0
        or len(entries.get("improve")) > 0
        or len(entries.get("fix")) > 0
    ):
        lines: list[str] = [
            "<h1>Changes for release</h1>",
            "<table>",
            "<tr><th>Category</th><th>Commit</th><th>Message</th></tr>",
        ]

        for category in [
            ("feature", "New Features"),
            ("improve", "Improvements"),
            ("fix", "Fixes"),
        ]:
            entry, title = category
            for i in range(len(entries.get(entry))):
                line = "<tr>"
                if i == 0:
                    line = f'{line}<td  style="vertical-align: top" rowspan="{len(entries.get(entry))}" style="vertical-align: top;"><b>{title}</b></td>'
                line = f"{line}{entries.get(entry)[i]}</tr>"
                lines.append(line)

        lines.append("</table>")
    else:
        lines.append("No changelog available")
        

    with open(CURRENT_VERSIONS_FILE, "r",encoding="utf-8") as f:
        current_versions:dict[str,str] = yaml.safe_load(f)
    with open(PREVIOUS_VERSIONS_FILE, "r",encoding="utf-8") as f:
        previous_versions:dict[str,str] = yaml.safe_load(f)

    diff_versions={}
    for cat in ["node","rust"]:
        diff_versions[cat]={}
        for p in current_versions[cat].keys():
            if p not in previous_versions[cat]:
                diff_versions[cat][p] = (None,current_versions[cat][p])
            elif previous_versions[cat][p]!=current_versions[cat][p]:
                diff_versions[cat][p] = (previous_versions[cat][p],current_versions[cat][p])
            else:
                diff_versions[cat][p] = (current_versions[cat][p],current_versions[cat][p])
        for p in previous_versions[cat].keys():
            if p not in current_versions[cat]:
                diff_versions[cat][p] = (previous_versions[cat][p],None)

    lines.append("<hr/>")

    lines.append("<h1>Dependencies</h1>")
    lines.append("<table>")
    lines.append("<tr><th>Language</th><th>Dependency</th><th>Action</th></tr>")

    urls = {"node":"https://www.npmjs.com/package/", "rust":"https://crates.io/crates/"}
    for cat in diff_versions.keys():
        if diff_versions[cat]:
            for (idx, package) in enumerate(sorted(diff_versions[cat])):
                line = "<tr>"
                if idx==0:
                    line = f'{line}<td  style="vertical-align: top" rowspan="{len(diff_versions[cat])}"><b>{cat.capitalize()}</b></td>'
                line = f'{line}<td><a href="{urls[cat]+package}" target="blank">{package}</a></td>'
                old = diff_versions[cat][package][0]
                new = diff_versions[cat][package][1]

                if old:
                    if new:
                        if old!=new:
                            line = f"{line}<td>{old} ➡️ {new}</td></tr>"
                        else:
                            line = f"{line}<td>{old}</td></tr>"
                    else:
                        line = f"{line}<td>🗑️ {old}</td></tr>"
                else:
                    line = f"{line}<td>✨ {new}</td></tr>"


                lines.append(line)
    lines.append("</table>")

    if os.path.exists(CHANGELOG_MD_FILE):
        os.unlink(CHANGELOG_MD_FILE)
    with open(CHANGELOG_MD_FILE, "w", encoding="utf-8") as f:
        f.write("\n".join(lines))


if __name__ == "__main__":
    create_dist_dir()
    generate_pkgbuild()
    generate_install()
    generate_srcinfo()
    generate_changelog()
    print("Release finished")

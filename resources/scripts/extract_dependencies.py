#!/bin/env python
"""
Requiere: pacman-contrib (para pactree)

Calcula el conjunto mínimo de paquetes pacman necesarios como
dependencia de un binario, a partir de sus librerías compartidas (ldd).
"""
import subprocess
from concurrent.futures import ThreadPoolExecutor

from commons import BIN_FILE, ENV_C

# Packages providing the commands the app spawns, which ldd cannot see:
# notify-send, sudo and xdg-open. udevadm, loginctl, runuser and flock come with base
COMMAND_DEPENDENCIES = ["libnotify", "sudo", "xdg-utils"]

def get_direct_deps(package: str) -> set[str]:
    """
    Obtiene las dependencias directas de un paquete usando pactree.
    """
    try:
        output = subprocess.check_output(
            ["pactree", "-d", "1", package],
            text=True,
            stderr=subprocess.DEVNULL,
        )
    except subprocess.CalledProcessError:
        return set()

    deps = set()
    lines = output.splitlines()

    # primera línea = paquete raíz
    for line in lines[1:]:
        dep = (
            line.replace("├─", "")
            .replace("└─", "")
            .replace("│", "")
            .strip()
        )
        if dep:
            deps.add(dep)

    return deps


def minimize_packages(packages: list[str]) -> list[str]:
    """
    Elimina paquetes que ya aparecen como dependencia
    de otro paquete de la lista.
    """
    packages = set(packages)
    all_transitive = set()

    # Cada llamada a get_direct_deps es un proceso pactree independiente:
    # se paralelizan con hilos (el cuello de botella es esperar al
    # subprocess, no CPU, así que el GIL no es un problema aquí).
    with ThreadPoolExecutor(max_workers=min(32, len(packages) or 1)) as pool:
        for deps in pool.map(get_direct_deps, packages):
            all_transitive.update(deps)

    roots = sorted(packages - all_transitive)
    return roots


def package_owners(so_files: list[str]) -> dict[str, str | None]:
    """
    Resuelve a qué paquete pertenece cada fichero en so_files con UNA
    sola llamada a pacman (en vez de una por fichero, que es lo que
    estaba haciendo de tiempo de ejecución antes).

    Devuelve {ruta: paquete}; paquete es None si la ruta no pertenece
    a ningún paquete instalado.
    """
    so_files = list(dict.fromkeys(so_files))  # dedup conservando orden
    if not so_files:
        return {}

    proc = subprocess.run(
        ["pacman", "-Qoq", *so_files],
        text=True,
        env=ENV_C,
        capture_output=True,
    )

    if proc.returncode == 0:
        # Caso normal: todas las rutas pertenecen a algún paquete.
        # pacman -Qoq devuelve una línea por fichero, en el mismo orden.
        owners = proc.stdout.splitlines()
        return dict(zip(so_files, owners))

    # Alguna ruta no pertenece a ningún paquete instalado. pacman sigue
    # procesando el resto, pero las que fallan no producen línea en
    # stdout (solo un error en stderr), así que hay que averiguar
    # cuáles fallaron para poder reasociar el resto por orden.
    prefix = "error: No package owns "
    failed = {
        line.strip()[len(prefix):].strip()
        for line in proc.stderr.splitlines()
        if line.strip().startswith(prefix)
    }

    ok_files = [f for f in so_files if f not in failed]
    owners = proc.stdout.splitlines()

    result = dict(zip(ok_files, owners))
    result.update({f: None for f in failed})
    return result


def do_extract() -> list[str]:
    if not BIN_FILE.exists():
        raise SystemExit(f"Binary does not exist: {BIN_FILE}")

    print("Getting all dependencies...")
    output = subprocess.check_output(["ldd", str(BIN_FILE)], text=True).splitlines()

    so_files: list[str] = []
    broken: list[str] = []

    for line in output:
        if " => " not in line:
            continue

        so_file = line.split(" => ")[1].split(" (")[0].strip()

        if not so_file or so_file == "not found":
            broken.append(line.strip())
            continue

        so_files.append(so_file)

    # Una sola llamada a pacman para resolver TODAS las rutas, en vez de
    # una llamada por cada una (esto es lo que más tiempo ahorra).
    owners = package_owners(so_files)

    dependencies: set[str] = set()
    unowned: list[str] = []

    for so_file in so_files:
        pkg = owners.get(so_file)
        if pkg is None:
            unowned.append(so_file)
        else:
            dependencies.add(pkg)

    if broken:
        print("WARNING: ldd could not resolve:")
        for item in broken:
            print(f"\t{item}")

    if unowned:
        print("WARNING: Following libraries are not owned by any package:")
        for item in unowned:
            print(f"\t{item}")

    dependencies.update(COMMAND_DEPENDENCIES)

    print("Minimizing...")
    dependencies = sorted(minimize_packages(list(dependencies)))

    for dep in dependencies:
        print(f"\t{dep}")

    return dependencies

if __name__ == "__main__":
    do_extract()
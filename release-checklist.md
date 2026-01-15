This is a little checklist for myself to ensure a release is done properly

1. CHANGELOG.md is updated with changes since previous version
2. Program version is bumped using ./bump-version.sh x.x.x
3. A tag is created (format: vx.x.x) -> all desired changes are pushed to origin -> tag is pushed to origin
4. A release is successfully created, all artifacts are there (deb, rpm, pkg.tar.zst, msi, dmg)
5. AUR is updated:

cd ~/aur/rimworld-workshop-downloader
cp ~/repos/rimworld-workshop-downloader/PKGBUILD .
makepkg --printsrcinfo > .SRCINFO
git commit -m "Release x.x.x"
git push
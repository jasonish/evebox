all:

# Commit the actual bower components used into version control so the
# project can be used as-is after being checked out from version
# control.
vendor-bower-components:
	sed -n 's/.*"\(bower_components.*\)\".*/\1/p' app/index.html | \
		(cd app && xargs git add -f)

	git add -f app/bower_components/bootstrap/dist/fonts

package:
	gulp package

# Basic clean - build artifacts, backup files...
clean:
	-gulp clean
	find . -name \*~ -exec rm -f {} \;

refresh-bower-components:
	rm -rf app/bower_components
	bower install

# Basic clean plus anything pulled down by build tools.
dist-clean: clean
	rm -rf node_modules
	rm -rf app/bower_components
	git checkout app/bower_components

prep:
	npm install
	bower install

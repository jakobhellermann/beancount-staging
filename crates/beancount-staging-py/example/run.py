import os

import beancount_staging

JOURNAL_FILES = ["docs/examples/journal.beancount"]
STAGING_FILES = ["docs/examples/staging.beancount"]

beancount_staging.serve(JOURNAL_FILES, STAGING_FILES)

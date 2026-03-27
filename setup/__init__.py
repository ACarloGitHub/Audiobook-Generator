# Retrocompatibilità: assicurati che la root del progetto sia in sys.path
# così 'setup.setup_helpers' viene trovato
import sys, os
_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if _root not in sys.path:
    sys.path.insert(0, _root)

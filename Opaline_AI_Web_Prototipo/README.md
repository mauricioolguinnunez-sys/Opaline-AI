# Opaline AI — Prototipo Web

Prototipo vacio (sin diseno) de la plataforma web Opaline AI.
Solo se implementa la conexion a LM Studio mediante FastAPI (comentada).

## Estructura
- frontend/ : vistas PHP + HTML vacias (sin CSS)
- backend/  : API FastAPI con conexion a LM Studio (localhost:1234)

## Ejecutar backend
cd backend
pip install -r requirements.txt
uvicorn main:app --reload --port 8000

## Ejecutar frontend
Servir frontend/ con PHP (php -S localhost:8080 -t frontend)

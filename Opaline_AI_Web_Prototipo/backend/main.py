"""
Opaline AI - Backend FastAPI
Conexion funcional (comentada) a LM Studio (localhost:1234).
El resto de endpoints son stubs vacios.
"""
from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel
import httpx
import os

app = FastAPI(title="Opaline AI API", version="0.1.0")

# CORS para permitir llamadas desde el frontend PHP/HTML
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

LM_STUDIO_URL = os.getenv("LM_STUDIO_URL", "http://localhost:1234")
LM_STUDIO_MODEL = os.getenv("LM_STUDIO_MODEL", "local-model")


class ChatRequest(BaseModel):
    message: str
    model: str | None = None
    history: list[dict] | None = None


class ChatResponse(BaseModel):
    reply: str
    model: str
    tokens: int | None = None


# ---------------------------------------------------------------------
# CONEXION A LM STUDIO (FUNCIONAL, COMENTADA)
# ---------------------------------------------------------------------
# LM Studio expone una API compatible con OpenAI en http://localhost:1234/v1
# Endpoint de chat: POST /v1/chat/completions
#
# Ejemplo de llamada directa con httpx:
#
# async with httpx.AsyncClient(timeout=180.0) as client:
#     payload = {
#         "model": LM_STUDIO_MODEL,
#         "messages": [
#             {"role": "system", "content": "Eres el asistente de Opaline AI."},
#             {"role": "user", "content": req.message},
#         ],
#         "temperature": 0.7,
#         "stream": False,
#     }
#     r = await client.post(f"{LM_STUDIO_URL}/v1/chat/completions", json=payload)
#     r.raise_for_status()
#     data = r.json()
#     reply = data["choices"][0]["message"]["content"]
#
# Para habilitar la conexion, descomente el bloque dentro de chat().


@app.get("/health")
def health():
    return {"status": "ok", "lm_studio_url": LM_STUDIO_URL}



@app.get("/api/v1/ai/models")
async def list_models():
    """Lista los modelos disponibles en LM Studio. Fallback a modelo local."""
    try:
        async with httpx.AsyncClient(timeout=10.0) as client:
            r = await client.get(f"{LM_STUDIO_URL}/v1/models")
            r.raise_for_status()
            data = r.json()
            models = [m.get("id") for m in data.get("data", []) if m.get("id")]
            if models:
                return {"models": models}
    except Exception:
        pass
    return {"models": [LM_STUDIO_MODEL]}


@app.post("/api/v1/ai/chat", response_model=ChatResponse)
async def chat(req: ChatRequest):
    """
    Endpoint de chat. Solo se conecta a LM Studio si se descomenta el bloque.
    Por defecto devuelve un eco para mantener el prototipo sin dependencias.
    """
    # Resolver modelo: si viene vacio o "local-model", usar el primero disponible en LM Studio.
    model = req.model
    if not model or model == "local-model":
        try:
            async with httpx.AsyncClient(timeout=10.0) as c:
                r = await c.get(f"{LM_STUDIO_URL}/v1/models")
                r.raise_for_status()
                data = r.json()
                ids = [m.get("id") for m in data.get("data", []) if m.get("id")]
                if ids:
                    model = ids[0]
        except Exception:
            model = LM_STUDIO_MODEL
    if not model:
        model = LM_STUDIO_MODEL

    # Conexion activa a LM Studio con fallback a eco si no esta disponible.
    try:
        async with httpx.AsyncClient(timeout=180.0) as client:
            msgs = [{"role": "system", "content": "Eres el asistente de Opaline AI."}]
            for h in (req.history or []):
                if h.get("role") in ("user", "assistant") and h.get("content"):
                    msgs.append({"role": h["role"], "content": h["content"]})
            msgs.append({"role": "user", "content": req.message})
            payload = {
                "model": model,
                "messages": msgs,
                "temperature": 0.7,
                "stream": False,
            }
            r = await client.post(f"{LM_STUDIO_URL}/v1/chat/completions", json=payload)
            r.raise_for_status()
            data = r.json()
            reply = data["choices"][0]["message"]["content"]
            tokens = data.get("usage", {}).get("total_tokens")
            return ChatResponse(reply=reply, model=model, tokens=tokens)
    except Exception:
        return ChatResponse(
            reply=f"[prototipo] Eco: {req.message}",
            model=model,
            tokens=None,
        )


# Stubs vacios de la API de negocio (PHP es la fuente de verdad)
@app.get("/api/v1/health")
def api_health():
    return {"status": "ok"}


if __name__ == "__main__":
    import uvicorn
    uvicorn.run("main:app", host="0.0.0.0", port=8000, reload=True)

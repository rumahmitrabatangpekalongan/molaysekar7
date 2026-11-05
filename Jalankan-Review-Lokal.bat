@echo off
chcp 65001 >nul
echo ==============================================================================
echo             MOLAY SEKAR7 - PR-AGENT / ZERO REGRESSION CODE REVIEW
echo ==============================================================================
echo Provider : SumoPod AI (https://ai.sumopod.com/v1)
echo Model    : glm-5.3-flash
echo ==============================================================================
echo.

python review_zero_regression.py

if exist hasil_review_lokal.md (
    echo.
    echo Membuka hasil review di editor/browser...
    start hasil_review_lokal.md
)

echo.
pause

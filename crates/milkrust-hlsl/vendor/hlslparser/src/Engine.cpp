
#include "Engine.h"
#include <sstream>
#include <iomanip>
#include <locale>
#include <limits>
#include <cmath>

#include <stdio.h>  // vsnprintf
#include <string.h> // strcmp, strcasecmp
#include <stdlib.h>	// strtod, strtol
#include <chrono>
#include <algorithm>
#include <string>
#include <memory>


namespace M4 {
namespace {
    thread_local size_t allocated = 0;
    thread_local unsigned steps = 0;
    thread_local unsigned depth = 0;
    thread_local std::chrono::steady_clock::time_point deadline;
    thread_local std::string diagnostics;
}
void ResetLimits() {
    allocated = 0; steps = 0; depth = 0; diagnostics.clear();
    deadline = std::chrono::steady_clock::now() + std::chrono::milliseconds(750);
}
void CheckBudget() {
    if (++steps > 2000000 || ((steps & 255) == 0 && std::chrono::steady_clock::now() > deadline))
        throw std::length_error("HLSL compilation budget exceeded");
}
void ChargeAllocation(size_t bytes) {
    CheckBudget();
    if (bytes > 32 * 1024 * 1024 - allocated) throw std::bad_alloc();
    allocated += bytes;
}
DepthGuard::DepthGuard() {
    CheckBudget();
    if (depth >= 192) throw std::length_error("HLSL recursion limit exceeded");
    ++depth;
}
DepthGuard::~DepthGuard() { --depth; }
const char* Diagnostics() { return diagnostics.empty() ? "HLSL compilation failed" : diagnostics.c_str(); }


// Engine/String.cpp

int String_PrintfArgList(char * buffer, int size, const char * format, va_list args) {

    va_list tmp;
    va_copy(tmp, args);

#if _MSC_VER >= 1400
	int n = vsnprintf_s(buffer, size, _TRUNCATE, format, tmp);
#else
	int n = vsnprintf(buffer, size, format, tmp);
#endif

    va_end(tmp);

	if (n < 0 || n >= size) throw std::length_error("HLSL formatted token limit");
	return n;
}

int String_Printf(char * buffer, int size, const char * format, ...) {

    va_list args;
    va_start(args, format);

    int n = String_PrintfArgList(buffer, size, format, args);

    va_end(args);

	return n;
}

int String_FormatFloat(char * buffer, int size, float value) {
    if (!std::isfinite(value)) throw std::range_error("Non-finite HLSL literal");
    std::ostringstream stream;
    stream.imbue(std::locale::classic());
    stream << std::setprecision(std::numeric_limits<float>::max_digits10) << value;
    std::string literal = stream.str();
    if (literal.find_first_of(".eE") == std::string::npos) literal += ".0";
    return String_Printf(buffer, size, "%s", literal.c_str());
}

bool String_Equal(const char * a, const char * b) {
	if (a == b) return true;
	if (a == NULL || b == NULL) return false;
	return strcmp(a, b) == 0;
}

bool String_EqualNoCase(const char * a, const char * b) {
	if (a == b) return true;
	if (a == NULL || b == NULL) return false;
#if _MSC_VER
	return _stricmp(a, b) == 0;
#else
	return strcasecmp(a, b) == 0;
#endif
}

double String_ToDouble(const char * str, char ** endptr) {
    const char* end = str;
    if (*end == '+' || *end == '-') ++end;
    bool digits = false;
    while (*end >= '0' && *end <= '9') { digits = true; ++end; }
    if (*end == '.') {
        ++end;
        while (*end >= '0' && *end <= '9') { digits = true; ++end; }
    }
    if (!digits) { *endptr = const_cast<char*>(str); return 0.0; }
    if (*end == 'e' || *end == 'E') {
        const char* exponent = end + 1;
        if (*exponent == '+' || *exponent == '-') ++exponent;
        const char* firstDigit = exponent;
        while (*exponent >= '0' && *exponent <= '9') ++exponent;
        if (exponent != firstDigit) end = exponent;
    }
    if (end - str > 1023) throw std::length_error("HLSL numeric literal limit");
    std::istringstream stream(std::string(str, end));
    stream.imbue(std::locale::classic());
    double value = 0;
    if (!(stream >> value)) throw std::range_error("Invalid HLSL floating literal");
    *endptr = const_cast<char*>(end);
    return value;
}

int String_ToInteger(const char * str, char ** endptr) {
	return strtol(str, endptr, 10);
}

int String_ToIntegerHex(const char * str, char ** endptr) {
	return strtol(str, endptr, 16);
}



// Engine/Log.cpp

void Log_Error(const char * format, ...) {
    va_list args;
    va_start(args, format);
    Log_ErrorArgList(format, args);
    va_end(args);
}

void Log_ErrorArgList(const char * format, va_list args) {
    char buffer[2048];
    vsnprintf(buffer, sizeof(buffer), format, args);
    if (diagnostics.size() < 8191) diagnostics.append(buffer, std::min(strlen(buffer), size_t(8191) - diagnostics.size()));
}


// Engine/StringPool.cpp

StringPool::StringPool(Allocator * allocator) : stringArray(allocator) {
}
StringPool::~StringPool() {
    for (int i = 0; i < stringArray.GetSize(); i++) {
        free((void *)stringArray[i]);
        stringArray[i] = NULL;
    }
}

const char * StringPool::AddString(const char * string) {
    for (int i = 0; i < stringArray.GetSize(); i++) {
        CheckBudget();
        if (String_Equal(stringArray[i], string)) return stringArray[i];
    }
ChargeAllocation(strlen(string) + 1);
#if _MSC_VER
    const char * dup = _strdup(string);
#else
    const char * dup = strdup(string);
#endif
    if (!dup) throw std::bad_alloc();
    try { stringArray.PushBack(dup); } catch (...) { free((void*)dup); throw; }
    return dup;
}

// @@ From mprintf.cpp
static char *mprintf_valist(int size, const char *fmt, va_list args) {
    ASSERT(size > 0);
    char *res = NULL;
    va_list tmp;

    while (1) {
        if (size <= 0 || size > 65536) throw std::length_error("HLSL token limit");
        ChargeAllocation(size);
        res = (char*)malloc(size);
        if (!res) throw std::bad_alloc();

        va_copy(tmp, args);
        int len = vsnprintf(res, size, fmt, tmp);
        va_end(tmp);

        if ((len >= 0) && (size >= len + 1)) {
            break;
        }

        free(res);

        if (len > -1 ) {
            size = len + 1;
        }
        else {
            size *= 2;
        }
    }

    return res;
}

const char * StringPool::AddStringFormatList(const char * format, va_list args) {
    va_list tmp;
    va_copy(tmp, args);
    std::unique_ptr<char, void(*)(void*)> owned(mprintf_valist(256, format, tmp), free);
    const char * string = owned.get();
    va_end(tmp);

    for (int i = 0; i < stringArray.GetSize(); i++) {
        CheckBudget();
        if (String_Equal(stringArray[i], string)) {
            return stringArray[i];
        }
    }

    stringArray.PushBack(string);
    owned.release();
    return string;
}

const char * StringPool::AddStringFormat(const char * format, ...) {
    va_list args;
    va_start(args, format);
    const char * string = AddStringFormatList(format, args);
    va_end(args);

    return string;
}

bool StringPool::GetContainsString(const char * string) const {
    for (int i = 0; i < stringArray.GetSize(); i++) {
        CheckBudget();
        if (String_Equal(stringArray[i], string)) return true;
    }
    return false;
}

} // M4 namespace

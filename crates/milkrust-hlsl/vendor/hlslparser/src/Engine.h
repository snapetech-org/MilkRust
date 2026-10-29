#ifndef ENGINE_H
#define ENGINE_H

#if _MSC_VER
#define _CRT_SECURE_NO_WARNINGS 1
#endif

#include <stdarg.h> // va_list, vsnprintf
#include <stdlib.h> // malloc
#include <new> // for placement new
#include <stdexcept>
#include <cstddef>

#ifndef NULL
#define NULL    0
#endif

#ifndef va_copy
#define va_copy(a, b) (a) = (b)
#endif

// Engine/Assert.h

#define ASSERT(...)
#define assert(...)

namespace M4 {

// MilkRust local hardening: per-thread compile budgets and bounded diagnostics.
void ResetLimits();
void CheckBudget();
void ChargeAllocation(size_t bytes);
const char* Diagnostics();
class DepthGuard {
public:
    DepthGuard();
    ~DepthGuard();
};



// Engine/Allocator.h

class Allocator {
public:
    template <typename T> T * New() {
        return New<T>(1);
    }
    template <typename T> T * New(size_t count) {
        if (count > size_t(-1) / sizeof(T)) throw std::bad_alloc();
        ChargeAllocation(sizeof(T) * count);
        T* result = (T *)malloc(sizeof(T) * count);
        if (!result) throw std::bad_alloc();
        return result;
    }
    template <typename T> void Delete(T * ptr) {
        free((void *)ptr);
    }
    template <typename T> T * Realloc(T * ptr, size_t count) {
        if (count > size_t(-1) / sizeof(T)) throw std::bad_alloc();
        ChargeAllocation(sizeof(T) * count);
        T* result = (T *)realloc(ptr, sizeof(T) * count);
        if (!result) throw std::bad_alloc();
        return result;
    }
};


// Engine/String.h

int String_Printf(char * buffer, int size, const char * format, ...);
int String_PrintfArgList(char * buffer, int size, const char * format, va_list args);
int String_FormatFloat(char * buffer, int size, float value);
bool String_Equal(const char * a, const char * b);
bool String_EqualNoCase(const char * a, const char * b);
double String_ToDouble(const char * str, char ** end);
int String_ToInteger(const char * str, char ** end);


// Engine/Log.h

void Log_Error(const char * format, ...);
void Log_ErrorArgList(const char * format, va_list args);


// Engine/Array.h

template <typename T>
void ConstructRange(T * buffer, int new_size, int old_size) {
    for (int i = old_size; i < new_size; i++) {
        new(buffer+i) T; // placement new
    }
}

template <typename T>
void ConstructRange(T * buffer, int new_size, int old_size, const T & val) {
    for (int i = old_size; i < new_size; i++) {
        new(buffer+i) T(val); // placement new
    }
}

template <typename T>
void DestroyRange(T * buffer, int new_size, int old_size) {
    for (int i = new_size; i < old_size; i++) {
        (buffer+i)->~T(); // Explicit call to the destructor
    }
}


template <typename T>
class Array {
public:
    Array(Allocator * allocator) : allocator(allocator), buffer(NULL), size(0), capacity(0) {}
    ~Array() { DestroyRange(buffer, 0, size); allocator->Delete(buffer); }
    Array(const Array&) = delete;
    Array& operator=(const Array&) = delete;

    void PushBack(const T & val) {
        ASSERT(&val < buffer || &val >= buffer+size);

        int old_size = size;
        int new_size = size + 1;

        SetSize(new_size);

        ConstructRange(buffer, new_size, old_size, val);
    }
    T & PushBackNew() {
        int old_size = size;
        int new_size = size + 1;

        SetSize(new_size);

        ConstructRange(buffer, new_size, old_size);

        return buffer[old_size];
    }
    void Resize(int new_size) {
        int old_size = size;

        DestroyRange(buffer, new_size, old_size);

        SetSize(new_size);

        ConstructRange(buffer, new_size, old_size);
    }

    int GetSize() const { return size; }
    const T & operator[](int i) const { if (i < 0 || i >= size) throw std::out_of_range("HLSL array index"); return buffer[i]; }
    T & operator[](int i) { if (i < 0 || i >= size) throw std::out_of_range("HLSL array index"); return buffer[i]; }

private:

    // Change array size.
    void SetSize(int new_size) {
        if (new_size < 0 || new_size > 262144) throw std::length_error("HLSL array limit");
        if (new_size > capacity) {
            int new_buffer_size;
            if (capacity == 0) {
                // first allocation is exact
                new_buffer_size = new_size;
            }
            else {
                // following allocations grow array by 25%
                new_buffer_size = new_size + (new_size >> 2);
            }

            SetCapacity(new_buffer_size);
        }
        size = new_size;
    }

    // Change array capacity.
    void SetCapacity(int new_capacity) {
        ASSERT(new_capacity >= size);

        if (new_capacity == 0) {
            // free the buffer.
            if (buffer != NULL) {
                allocator->Delete<T>(buffer);
                buffer = NULL;
            }
        }
        else {
            // realloc the buffer
            buffer = allocator->Realloc<T>(buffer, new_capacity);
        }

        capacity = new_capacity;
    }


private:
    Allocator * allocator; // @@ Do we really have to keep a pointer to this?
    T * buffer;
    int size;
    int capacity;
};


// Engine/StringPool.h

// @@ Implement this with a hash table!
struct StringPool {
    StringPool(Allocator * allocator);
    ~StringPool();

    const char * AddString(const char * string);
    const char * AddStringFormat(const char * fmt, ...);
    const char * AddStringFormatList(const char * fmt, va_list args);
    bool GetContainsString(const char * string) const;

    Array<const char *> stringArray;
};


} // M4 namespace

#endif // ENGINE_H


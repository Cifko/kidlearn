import React, { ReactNode, useState } from 'react';
import {
  ActivityIndicator,
  Image,
  Modal,
  SafeAreaView,
  ScrollView,
  ScrollViewInstance,
  StyleSheet,
  Text,
  TouchableOpacity,
  View,
} from 'react-native';
import { StatusBar } from 'expo-status-bar';
import DefaultBackground from '../assets/background.png';
import { colors } from './theme';

export function Screen({
  children,
  scrollRef,
  onViewportHeight,
  backgroundUri,
}: {
  children: ReactNode;
  scrollRef?: React.Ref<ScrollViewInstance>;
  onViewportHeight?: (height: number) => void;
  backgroundUri?: string | null;
}) {
  return (
    <SafeAreaView style={[styles.safe, backgroundUri != null && styles.transparent]}>
      <StatusBar style="dark" />
      {backgroundUri != null ? (
        <Image
          source={{ uri: backgroundUri }}
          style={styles.background}
          resizeMode="cover"
          pointerEvents="none"
        />
      ) : (
        <Image
          source={DefaultBackground}
          style={styles.defaultBackground}
          resizeMode="cover"
          pointerEvents="none"
        />
      )}
      <ScrollView
        ref={scrollRef}
        onLayout={
          onViewportHeight
            ? (event) => onViewportHeight(event.nativeEvent.layout.height)
            : undefined
        }
        contentContainerStyle={styles.content}
        keyboardShouldPersistTaps="handled"
      >
        {children}
      </ScrollView>
    </SafeAreaView>
  );
}

type ButtonColor = 'green' | 'blue' | 'red' | 'gray' | 'facebook';

const buttonColors: Record<ButtonColor, { bg: string; edge: string }> = {
  green: { bg: colors.green, edge: colors.greenDark },
  blue: { bg: colors.blue, edge: colors.blueDark },
  red: { bg: colors.red, edge: colors.redDark },
  gray: { bg: colors.gray, edge: colors.grayDark },
  // Official Facebook blue; the edge is our chunky shading, not branding.
  facebook: { bg: '#1877F2', edge: '#1462C6' },
};

export function ChunkyButton({
  title,
  onPress,
  color = 'green',
  disabled = false,
  loading = false,
  icon,
}: {
  title: string;
  onPress: () => void;
  color?: ButtonColor;
  disabled?: boolean;
  loading?: boolean;
  icon?: ReactNode;
}) {
  const scheme = disabled ? buttonColors.gray : buttonColors[color];
  return (
    <TouchableOpacity
      onPress={onPress}
      disabled={disabled || loading}
      style={[styles.button, { backgroundColor: scheme.bg, borderBottomColor: scheme.edge }]}
    >
      {loading ? (
        <ActivityIndicator color="#FFFFFF" />
      ) : icon ? (
        <View style={styles.buttonContent}>
          {icon}
          <Text style={styles.buttonText}>{title}</Text>
        </View>
      ) : (
        <Text style={styles.buttonText}>{title}</Text>
      )}
    </TouchableOpacity>
  );
}

export function Segmented<T extends string>({
  options,
  value,
  onChange,
}: {
  options: { value: T; label: string }[];
  value: T;
  onChange: (value: T) => void;
}) {
  return (
    <View style={styles.segmented}>
      {options.map((option) => {
        const active = option.value === value;
        return (
          <TouchableOpacity
            key={option.value}
            onPress={() => onChange(option.value)}
            style={[styles.segment, active && styles.segmentActive]}
          >
            <Text style={[styles.segmentText, active && styles.segmentTextActive]}>
              {option.label}
            </Text>
          </TouchableOpacity>
        );
      })}
    </View>
  );
}

export function Dropdown<T extends string>({
  options,
  value,
  onChange,
}: {
  options: { value: T; label: string }[];
  value: T;
  onChange: (value: T) => void;
}) {
  const [open, setOpen] = useState(false);
  const selected = options.find((option) => option.value === value);
  return (
    <>
      <TouchableOpacity onPress={() => setOpen(true)} style={styles.dropdownButton}>
        <Text style={styles.dropdownButtonText}>{selected?.label ?? value}</Text>
        <Text style={styles.dropdownCaret}>▾</Text>
      </TouchableOpacity>
      <Modal
        visible={open}
        transparent
        animationType="fade"
        onRequestClose={() => setOpen(false)}
      >
        <View style={styles.dropdownBackdrop}>
          {/* Sibling, not parent, of the sheet: taps on rows must not close it. */}
          <TouchableOpacity
            style={StyleSheet.absoluteFill}
            activeOpacity={1}
            onPress={() => setOpen(false)}
          />
          <View style={styles.dropdownSheet}>
            <ScrollView keyboardShouldPersistTaps="handled">
              {options.map((option) => {
                const active = option.value === value;
                return (
                  <TouchableOpacity
                    key={option.value}
                    onPress={() => {
                      onChange(option.value);
                      setOpen(false);
                    }}
                    style={[styles.dropdownRow, active && styles.dropdownRowActive]}
                  >
                    <Text
                      style={[styles.dropdownRowText, active && styles.dropdownRowTextActive]}
                    >
                      {option.label}
                    </Text>
                    {active && <Text style={styles.dropdownCheck}>✓</Text>}
                  </TouchableOpacity>
                );
              })}
            </ScrollView>
          </View>
        </View>
      </Modal>
    </>
  );
}

export function Stepper({
  value,
  min,
  max,
  onChange,
}: {
  value: number;
  min: number;
  max: number;
  onChange: (value: number) => void;
}) {
  return (
    <View style={styles.stepper}>
      <TouchableOpacity
        onPress={() => onChange(Math.max(min, value - 1))}
        disabled={value <= min}
        style={[styles.stepperButton, value <= min && styles.stepperDisabled]}
      >
        <Text style={styles.stepperText}>−</Text>
      </TouchableOpacity>
      <Text style={styles.stepperValue}>{value}</Text>
      <TouchableOpacity
        onPress={() => onChange(Math.min(max, value + 1))}
        disabled={value >= max}
        style={[styles.stepperButton, value >= max && styles.stepperDisabled]}
      >
        <Text style={styles.stepperText}>+</Text>
      </TouchableOpacity>
    </View>
  );
}

export function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <View style={styles.field}>
      <Text style={styles.fieldLabel}>{label}</Text>
      {children}
    </View>
  );
}

export function ErrorBox({ message, onRetry }: { message: string; onRetry?: () => void }) {
  return (
    <View style={styles.errorBox}>
      <Text style={styles.errorText}>{message}</Text>
      {onRetry && (
        <TouchableOpacity onPress={onRetry} style={styles.errorRetry}>
          <Text style={styles.errorRetryText}>↻</Text>
        </TouchableOpacity>
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  safe: {
    flex: 1,
    backgroundColor: colors.bg,
  },
  transparent: {
    backgroundColor: 'transparent',
  },
  background: {
    position: 'absolute',
    top: 0,
    left: 0,
    right: 0,
    bottom: 0,
    opacity: 0.45,
  },
  // The bundled art is already faint, so it renders opaque; the AI
  // subject backgrounds above stay dimmed to keep text readable.
  defaultBackground: {
    position: 'absolute',
    top: 0,
    left: 0,
    right: 0,
    bottom: 0,
  },
  content: {
    padding: 20,
    width: '100%',
    maxWidth: 640,
    alignSelf: 'center',
  },
  button: {
    borderRadius: 12,
    borderBottomWidth: 4,
    paddingVertical: 14,
    alignItems: 'center',
  },
  buttonText: {
    color: '#FFFFFF',
    fontSize: 16,
    fontWeight: '800',
    letterSpacing: 1,
  },
  buttonContent: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: 10,
  },
  segmented: {
    flexDirection: 'row',
    backgroundColor: colors.card,
    borderRadius: 12,
    borderWidth: 2,
    borderColor: colors.border,
    padding: 4,
    gap: 4,
  },
  segment: {
    flex: 1,
    borderRadius: 8,
    paddingVertical: 10,
    alignItems: 'center',
  },
  segmentActive: {
    backgroundColor: '#FFFFFF',
    borderWidth: 2,
    borderColor: colors.border,
  },
  segmentText: {
    fontSize: 15,
    fontWeight: '700',
    color: colors.muted,
  },
  segmentTextActive: {
    color: colors.blue,
  },
  dropdownButton: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    backgroundColor: '#FFFFFF',
    borderRadius: 12,
    borderWidth: 2,
    borderColor: colors.border,
    paddingHorizontal: 14,
    paddingVertical: 12,
  },
  dropdownButtonText: {
    fontSize: 16,
    color: colors.ink,
  },
  dropdownCaret: {
    fontSize: 16,
    color: colors.muted,
  },
  dropdownBackdrop: {
    flex: 1,
    backgroundColor: 'rgba(0, 0, 0, 0.35)',
    justifyContent: 'center',
    padding: 32,
  },
  dropdownSheet: {
    backgroundColor: '#FFFFFF',
    borderRadius: 16,
    maxHeight: 420,
    paddingVertical: 8,
  },
  dropdownRow: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingVertical: 12,
    paddingHorizontal: 16,
  },
  dropdownRowActive: {
    backgroundColor: '#EEF6FF',
  },
  dropdownRowText: {
    fontSize: 16,
    fontWeight: '600',
    color: colors.ink,
  },
  dropdownRowTextActive: {
    fontWeight: '800',
    color: colors.blueDark,
  },
  dropdownCheck: {
    fontSize: 16,
    fontWeight: '800',
    color: colors.blue,
  },
  stepper: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 16,
  },
  stepperButton: {
    width: 44,
    height: 44,
    borderRadius: 12,
    backgroundColor: colors.card,
    borderWidth: 2,
    borderColor: colors.border,
    alignItems: 'center',
    justifyContent: 'center',
  },
  stepperDisabled: {
    opacity: 0.4,
  },
  stepperText: {
    fontSize: 22,
    fontWeight: '800',
    color: colors.ink,
  },
  stepperValue: {
    fontSize: 24,
    fontWeight: '800',
    color: colors.ink,
    minWidth: 48,
    textAlign: 'center',
  },
  field: {
    marginBottom: 18,
  },
  fieldLabel: {
    fontSize: 14,
    fontWeight: '800',
    color: colors.muted,
    marginBottom: 8,
    letterSpacing: 0.5,
  },
  errorBox: {
    backgroundColor: '#FFE5E5',
    borderRadius: 12,
    padding: 12,
    marginBottom: 16,
    flexDirection: 'row',
    alignItems: 'center',
    gap: 8,
  },
  errorText: {
    flex: 1,
    color: colors.redDark,
    fontSize: 14,
    fontWeight: '600',
  },
  errorRetry: {
    padding: 4,
  },
  errorRetryText: {
    fontSize: 18,
    color: colors.redDark,
    fontWeight: '800',
  },
});
